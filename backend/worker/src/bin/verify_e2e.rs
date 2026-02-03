use anyhow::Result;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client;
use common::entities::{prelude::*, *};
use common::settings::Settings;
use sea_orm::*;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Setup Settings
    let settings = Settings::new().expect("Failed to load configuration");

    println!("=== E2E Verification Start ===");

    // 2. Connect DB
    let db_url = settings.database.url.clone();
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

    // 5. Setup S3 Client (using settings)
    let bucket = settings.s3.bucket.clone();
    let endpoint = settings.s3.endpoint.clone();
    let region = settings.s3.region.clone();
    let access_key = settings.s3.access_key_id.clone();
    let secret_key = settings.s3.secret_access_key.clone();

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

    if let (Some(ak), Some(sk)) = (access_key, secret_key) {
        let creds = aws_sdk_s3::config::Credentials::new(ak, sk, None, None, "config");
        config_loader = config_loader
            .credentials_provider(aws_sdk_s3::config::SharedCredentialsProvider::new(creds));
    }

    let config = config_loader.load().await;

    // Determine path style
    let is_aliyun = endpoint
        .as_deref()
        .map(|e| e.contains("aliyuncs.com"))
        .unwrap_or(false);
    let endpoint_present = endpoint.is_some();

    // Use settings force_path_style, or fallback logic if it was false (to be safe, or just trust settings)
    // We will trust settings if true, otherwise fallback to inference for safety in this script
    let force_path_style = if settings.s3.force_path_style {
        true
    } else {
        endpoint_present && !is_aliyun
    };

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
