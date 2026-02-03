use anyhow::Context;
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use aws_sdk_s3::config::{Credentials, SharedCredentialsProvider};
use chrono::Utc;
use common::entities::prelude::Users;
use common::entities::{auth_identities, local_credentials, users};
use common::settings::Settings;
use migration::{Migrator, MigratorTrait};
use rand::rngs::OsRng;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use std::str::FromStr;
use std::time::Duration;
use temporalio_client::{ClientOptions, WorkflowClientTrait, WorkflowOptions};
use temporalio_sdk_core::Url;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting Setup...");

    let settings = Settings::new().context("Failed to load config")?;

    // 2. Database Setup
    let db_url = settings.database.url.clone();

    let db = wait_for_db(&db_url).await?;

    tracing::info!("Running migrations...");
    Migrator::up(&db, None).await?;
    tracing::info!("Migrations applied.");

    seed_admin(&db, &settings).await?;

    // 3. S3 Setup
    setup_s3(&settings).await?;

    // 4. Temporal Setup
    setup_temporal(&settings).await?;

    tracing::info!("Setup completed successfully!");
    Ok(())
}

async fn seed_admin(db: &DatabaseConnection, settings: &Settings) -> anyhow::Result<()> {
    let existing_admin = Users::find()
        .filter(users::Column::Role.eq(users::UserRole::Admin))
        .one(db)
        .await?;

    if existing_admin.is_some() {
        return Ok(());
    }

    let username = std::env::var("SKILLREGISTRY_ADMIN_USERNAME")
        .ok()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| settings.auth.admin_bootstrap.username.trim().to_lowercase());

    let password = std::env::var("SKILLREGISTRY_ADMIN_PASSWORD")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| settings.auth.admin_bootstrap.password.clone())
        .unwrap_or_else(|| {
            if settings.debug {
                "admin".to_string()
            } else {
                "".to_string()
            }
        });

    if password.is_empty() {
        return Err(anyhow::anyhow!(
            "admin bootstrap password is required when debug=false"
        ));
    }

    let now = Utc::now().naive_utc();
    let user_id = Uuid::new_v4();

    let username_taken = Users::find()
        .filter(users::Column::Username.eq(username.clone()))
        .one(db)
        .await?
        .is_some();

    if username_taken {
        return Err(anyhow::anyhow!("admin username already exists"));
    }

    users::ActiveModel {
        user_id: Set(user_id),
        status: Set(users::UserStatus::Active),
        role: Set(users::UserRole::Admin),
        username: Set(Some(username.clone())),
        display_name: Set(Some("Admin".to_string())),
        primary_email: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await?;

    auth_identities::ActiveModel {
        user_id: Set(user_id),
        provider: Set(auth_identities::AuthProvider::Local),
        provider_user_id: Set(username),
        email: Set(None),
        email_verified: Set(false),
        display_name: Set(Some("Admin".to_string())),
        created_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await?;

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("failed to hash admin password: {}", e))?
        .to_string();

    local_credentials::ActiveModel {
        user_id: Set(user_id),
        password_hash: Set(password_hash),
        password_updated_at: Set(now),
    }
    .insert(db)
    .await?;

    Ok(())
}

async fn wait_for_db(url: &str) -> anyhow::Result<DatabaseConnection> {
    tracing::info!("Connecting to database at {}...", url);
    let mut attempt = 1;
    loop {
        match Database::connect(url).await {
            Ok(db) => {
                tracing::info!("Database connected!");
                return Ok(db);
            }
            Err(e) => {
                if attempt > 30 {
                    return Err(anyhow::anyhow!(
                        "Failed to connect to DB after 30 attempts: {}",
                        e
                    ));
                }
                tracing::warn!(
                    "Failed to connect to DB (attempt {}): {}. Retrying in 2s...",
                    attempt,
                    e
                );
                tokio::time::sleep(Duration::from_secs(2)).await;
                attempt += 1;
            }
        }
    }
}

async fn setup_s3(settings: &Settings) -> anyhow::Result<()> {
    let bucket_name = settings.s3.bucket.clone();
    let endpoint = settings.s3.endpoint.clone();
    let region = settings.s3.region.clone();
    let access_key = settings.s3.access_key_id.clone().unwrap_or_default();
    let secret_key = settings.s3.secret_access_key.clone().unwrap_or_default();

    tracing::info!("Setting up S3 (Bucket: {})...", bucket_name);

    let mut config_builder = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new(region.clone()));

    // Apply credentials if present
    if !access_key.is_empty() && !secret_key.is_empty() {
        let creds = Credentials::new(access_key, secret_key, None, None, "config");
        config_builder = config_builder.credentials_provider(SharedCredentialsProvider::new(creds));
    }

    if let Some(ep) = endpoint.clone() {
        let ep = ep.trim_matches('"').to_string();
        let ep = if ep.starts_with("http") {
            ep
        } else {
            format!("https://{}", ep)
        };
        config_builder = config_builder.endpoint_url(ep);
    }

    let sdk_config = config_builder.load().await;
    let mut s3_conf_builder = aws_sdk_s3::config::Builder::from(&sdk_config);
    if endpoint.is_some() {
        s3_conf_builder = s3_conf_builder.force_path_style(true);
    }
    let client = aws_sdk_s3::Client::from_conf(s3_conf_builder.build());

    // Wait for S3
    let mut attempt = 1;
    loop {
        match client.list_buckets().send().await {
            Ok(_) => break,
            Err(e) => {
                if attempt > 30 {
                    return Err(anyhow::anyhow!(
                        "Failed to connect to S3 after 30 attempts: {}",
                        e
                    ));
                }
                tracing::warn!("Waiting for S3 (attempt {})...", attempt);
                tokio::time::sleep(Duration::from_secs(2)).await;
                attempt += 1;
            }
        }
    }

    // Create Bucket
    match client.create_bucket().bucket(&bucket_name).send().await {
        Ok(_) => tracing::info!("Bucket {} created.", bucket_name),
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("BucketAlreadyOwnedByYou")
                || err_str.contains("BucketAlreadyExists")
            {
                tracing::info!("Bucket {} already exists.", bucket_name);
            } else {
                tracing::warn!("Failed to create bucket (might already exist): {}", e);
            }
        }
    }

    Ok(())
}

async fn setup_temporal(settings: &Settings) -> anyhow::Result<()> {
    let server_url = settings.temporal.server_url.clone();
    let task_queue = settings.temporal.task_queue.as_str();

    tracing::info!("Connecting to Temporal at {}...", server_url);

    let client_options = ClientOptions::builder()
        .target_url(Url::from_str(&server_url)?)
        .client_name("setup-tool")
        .client_version("0.1.0")
        .build();

    // Wait for Temporal
    let mut attempt = 1;
    let client = loop {
        match client_options.connect("default", None).await {
            Ok(c) => break c,
            Err(e) => {
                if attempt > 30 {
                    return Err(anyhow::anyhow!(
                        "Failed to connect to Temporal after 30 attempts: {}",
                        e
                    ));
                }
                tracing::warn!("Waiting for Temporal (attempt {})...", attempt);
                tokio::time::sleep(Duration::from_secs(2)).await;
                attempt += 1;
            }
        }
    };

    tracing::info!("Temporal connected. Registering Schedules/Workflows...");

    // Start Discovery Workflow (Run every 1 hour)
    let discovery_id = "discovery-periodic";
    let opts = WorkflowOptions {
        cron_schedule: Some("0 * * * *".to_string()), // Every hour
        ..Default::default()
    };

    match client
        .start_workflow(
            vec![],
            task_queue.to_string(),
            discovery_id.to_string(),
            "discovery_workflow".to_string(),
            None,
            opts,
        )
        .await
    {
        Ok(_) => tracing::info!("Started {} with cron.", discovery_id),
        Err(e) => tracing::warn!("Failed to start {} (might be running): {}", discovery_id, e),
    }

    // Start Sync Scheduler Workflow (Run every 10 minutes)
    let sync_id = "sync-scheduler-periodic";
    let sync_opts = WorkflowOptions {
        cron_schedule: Some("*/10 * * * *".to_string()), // Every 10 mins
        ..Default::default()
    };
    match client
        .start_workflow(
            vec![],
            task_queue.to_string(),
            sync_id.to_string(),
            "sync_scheduler_workflow".to_string(),
            None,
            sync_opts,
        )
        .await
    {
        Ok(_) => tracing::info!("Started {} with cron.", sync_id),
        Err(e) => tracing::warn!("Failed to start {} (might be running): {}", sync_id, e),
    }

    Ok(())
}
