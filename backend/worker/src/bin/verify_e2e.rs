use anyhow::Result;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client;
use common::entities::{prelude::*, *};
use sea_orm::*;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Setup Env
    if dotenv::dotenv().is_err() {
        // Try looking up
        if let Ok(cwd) = std::env::current_dir() {
            let candidates = [
                cwd.join(".env"),
                cwd.join("../.env"),
                cwd.join("../../.env"),
            ];
            for p in candidates {
                if p.exists() && dotenv::from_path(&p).is_ok() {
                    break;
                }
            }
        }
    }

    println!("=== E2E Verification Start ===");

    // 2. Connect DB
    let db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://skillregistry.db?mode=rwc".to_string());
    println!("Connecting to DB: {}", db_url);
    let db = Database::connect(&db_url).await?;

    // 3. Check Counts
    let registry_count = SkillRegistry::find().count(&db).await?;
    println!("Discovered Repositories: {}", registry_count);

    let skills_count = Skills::find().count(&db).await?;
    println!("Synced Skills: {}", skills_count);

    let versions_count = SkillVersions::find().count(&db).await?;
    println!("Skill Versions: {}", versions_count);

    if skills_count == 0 {
        println!("!!! No skills synced. Aborting S3 check. !!!");
        return Ok(());
    }

    // 4. Verify One Skill
    let version = SkillVersions::find()
        .order_by_desc(skill_versions::Column::CreatedAt)
        .one(&db)
        .await?
        .expect("Should have at least one version if count > 0");

    let s3_key = version.s3_key.expect("Version should have s3_key");
    println!("Verifying S3 Key: {}", s3_key);

    // 5. Setup S3 Client
    let bucket = env::var("S3_BUCKET")
        .or_else(|_| env::var("S3_BUCKET_NAME"))
        .expect("S3_BUCKET not set");
    let endpoint = env::var("S3_ENDPOINT")
        .ok()
        .or_else(|| env::var("S3_ENDPOINT_URL").ok());
    let region = env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());

    let region_provider =
        RegionProviderChain::first_try(aws_config::Region::new(region)).or_default_provider();
    let mut config_loader =
        aws_config::defaults(aws_config::BehaviorVersion::latest()).region(region_provider);

    if let Some(ep) = &endpoint {
        let ep = ep.trim_matches('"').to_string();
        let ep = if ep.starts_with("http") {
            ep
        } else {
            format!("https://{}", ep)
        };
        config_loader = config_loader.endpoint_url(ep);
    }

    // Explicitly set credentials from env if present (as in common/src/s3.rs)
    let access_key_id = std::env::var("S3_ACCESS_KEY_ID")
        .ok()
        .or_else(|| std::env::var("AWS_ACCESS_KEY_ID").ok());
    let secret_access_key = std::env::var("S3_ACCESS_KEY_SECRET")
        .ok()
        .or_else(|| std::env::var("AWS_SECRET_ACCESS_KEY").ok());

    if let (Some(ak), Some(sk)) = (access_key_id, secret_access_key) {
        let creds = aws_credential_types::Credentials::new(ak, sk, None, None, "env");
        config_loader = config_loader.credentials_provider(
            aws_credential_types::provider::SharedCredentialsProvider::new(creds),
        );
    }

    let config = config_loader.load().await;
    let mut force_path_style = true;
    if let Some(ep) = &endpoint {
        if ep.contains("aliyuncs.com") {
            force_path_style = false;
        }
    }

    let s3_config = aws_sdk_s3::config::Builder::from(&config)
        .force_path_style(force_path_style)
        .build();
    let client = Client::from_conf(s3_config);

    // 6. Check Object
    match client
        .head_object()
        .bucket(&bucket)
        .key(&s3_key)
        .send()
        .await
    {
        Ok(_) => println!("✅ S3 Verification SUCCESS: Object exists."),
        Err(e) => {
            println!("❌ S3 Verification FAILED: {}", e);
            // Try list objects to see what's there
            println!("Listing bucket contents:");
            let list = client.list_objects_v2().bucket(&bucket).send().await;
            match list {
                Ok(out) => {
                    for obj in out.contents() {
                        println!(" - {}", obj.key().unwrap_or("<none>"));
                    }
                }
                Err(le) => println!("Failed to list bucket: {}", le),
            }
        }
    }

    Ok(())
}
