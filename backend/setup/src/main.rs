use anyhow::Context;
use common::config::AppConfig;
use migration::{Migrator, MigratorTrait};
use sea_orm::{Database, DatabaseConnection};
use std::str::FromStr;
use std::time::Duration;
use temporalio_client::{ClientOptions, WorkflowClientTrait, WorkflowOptions};
use temporalio_sdk_core::Url;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Initialize
    if dotenv::dotenv().is_err() {
        tracing::warn!("No .env file found");
    }

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting Setup...");

    let config = AppConfig::new().context("Failed to load config")?;

    // 2. Database Setup
    let db_url = std::env::var("DATABASE_URL")
        .ok()
        .or(config.database.as_ref().and_then(|d| d.url.clone()))
        .unwrap_or_else(|| "postgres://postgres:postgres@localhost:5432/skillregistry".to_string());

    let db = wait_for_db(&db_url).await?;

    tracing::info!("Running migrations...");
    Migrator::up(&db, None).await?;
    tracing::info!("Migrations applied.");

    // 3. S3 Setup
    setup_s3(&config).await?;

    // 4. Temporal Setup
    setup_temporal().await?;

    tracing::info!("Setup completed successfully!");
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

async fn setup_s3(config: &AppConfig) -> anyhow::Result<()> {
    let bucket_name = std::env::var("S3_BUCKET")
        .or_else(|_| std::env::var("S3_BUCKET_NAME"))
        .unwrap_or_else(|_| config.s3.bucket.clone());

    let endpoint = std::env::var("S3_ENDPOINT")
        .ok()
        .or_else(|| std::env::var("S3_ENDPOINT_URL").ok())
        .or(config.s3.endpoint.clone());

    let region = std::env::var("AWS_REGION")
        .or_else(|_| std::env::var("S3_REGION"))
        .unwrap_or_else(|_| config.s3.region.clone());

    let _access_key = std::env::var("AWS_ACCESS_KEY_ID").unwrap_or_default();
    let _secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").unwrap_or_default();

    tracing::info!("Setting up S3 (Bucket: {})...", bucket_name);

    let mut config_builder = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new(region.clone()));

    if let Some(ep) = endpoint {
        let ep = ep.trim_matches('"').to_string();
        let ep = if ep.starts_with("http") {
            ep
        } else {
            format!("https://{}", ep)
        };
        config_builder = config_builder.endpoint_url(ep);
    }

    // In local/minio/rustfs, we might need force_path_style = true if not handled by endpoint
    // But aws-config generic setup usually handles it if endpoint is provided.
    // We use aws-sdk-s3 directly.

    let sdk_config = config_builder.load().await;
    let client = aws_sdk_s3::Client::new(&sdk_config);

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

async fn setup_temporal() -> anyhow::Result<()> {
    let server_url = std::env::var("TEMPORAL_SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:7233".to_string());
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
            "skill-registry-queue".to_string(),
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
            "skill-registry-queue".to_string(),
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
