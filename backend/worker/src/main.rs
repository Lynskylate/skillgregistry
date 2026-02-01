mod github;
mod tasks;
mod ports;

use std::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use common::db;
use common::s3::S3Service;
use common::config::AppConfig;
use common::entities::{prelude::*, *};
use sea_orm::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if dotenv::dotenv().is_err() {
        if let Ok(cwd) = std::env::current_dir() {
            let candidates = [
                cwd.join(".env"),
                cwd.join("../.env"),
                cwd.join("../../.env"),
                cwd.join("../../../.env"),
            ];
            for p in candidates {
                if p.exists() && dotenv::from_path(&p).is_ok() {
                    break;
                }
            }
        }
    }
    
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "worker=debug,common=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::new().expect("Failed to load configuration");

    let db_url = std::env::var("DATABASE_URL").ok()
        .or(config.database.as_ref().and_then(|d| d.url.clone()))
        .unwrap_or_else(|| "sqlite://skillregistry.db?mode=rwc".to_string());
        
    let db = db::establish_connection(&db_url).await?;
    
    let s3_bucket = std::env::var("S3_BUCKET")
        .or_else(|_| std::env::var("S3_BUCKET_NAME"))
        .unwrap_or_else(|_| config.s3.bucket);

    let s3_endpoint = std::env::var("S3_ENDPOINT")
        .ok()
        .or_else(|| std::env::var("S3_ENDPOINT_URL").ok())
        .or_else(|| std::env::var("AWS_ENDPOINT_URL").ok())
        .or(config.s3.endpoint);

    let inferred_region = s3_endpoint.as_deref().and_then(|ep| {
        let ep = ep.trim_matches('"');
        let ep = ep.strip_prefix("https://").or_else(|| ep.strip_prefix("http://")).unwrap_or(ep);
        let prefix = "oss-";
        let suffix = ".aliyuncs.com";
        if let Some(pos) = ep.find(prefix) {
            let rest = &ep[(pos + prefix.len())..];
            if let Some(end) = rest.find(suffix) {
                return Some(rest[..end].to_string());
            }
        }
        None
    });

    let s3_region = std::env::var("AWS_REGION")
        .or_else(|_| std::env::var("S3_REGION"))
        .ok()
        .or(inferred_region)
        .unwrap_or_else(|| config.s3.region);

    tracing::info!(
        s3_bucket = %s3_bucket,
        s3_region = %s3_region,
        s3_endpoint = %s3_endpoint.clone().unwrap_or_else(|| "<aws-default>".to_string()),
        "S3 config loaded"
    );

    let s3 = S3Service::new(s3_bucket, s3_region, s3_endpoint).await;
    
    let github_token = std::env::var("GITHUB_TOKEN").ok();
    let github = github::GithubClient::new(github_token);

    let keywords_str = std::env::var("SEARCH_KEYWORDS").unwrap_or_else(|_| "topic:agent-skill".to_string());
    let queries: Vec<String> = keywords_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

    tracing::info!("Worker started");

    loop {
        run_task("discovery", &db, || {
            let q = queries.clone();
            tasks::discovery::run(&db, &github, q)
        }).await;
        run_task("sync", &db, || tasks::sync::run(&db, &s3, &github)).await;

        let interval = config.worker.scan_interval_seconds;
            
        tracing::info!("Sleeping for {} seconds...", interval);
        tokio::time::sleep(Duration::from_secs(interval)).await;
    }
}

async fn run_task<F, Fut>(name: &str, db: &DatabaseConnection, task_fn: F)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    tracing::info!("Running {} task...", name);
    let start_time = chrono::Utc::now().naive_utc();
    
    // Create log entry
    let log_entry = task_logs::ActiveModel {
        task_name: Set(name.to_string()),
        status: Set("running".to_string()),
        started_at: Set(start_time),
        ..Default::default()
    };
    let log_res = log_entry.insert(db).await;
    let log_id = match log_res {
        Ok(l) => Some(l.id),
        Err(e) => {
            tracing::error!("Failed to create task log: {}", e);
            None
        }
    };

    let result = task_fn().await;
    let end_time = chrono::Utc::now().naive_utc();

    if let Some(id) = log_id {
        let (status, details) = match &result {
            Ok(_) => ("success".to_string(), None),
            Err(e) => ("failed".to_string(), Some(e.to_string())),
        };

        let update = task_logs::ActiveModel {
            id: Set(id),
            status: Set(status),
            details: Set(details),
            ended_at: Set(Some(end_time)),
            ..Default::default()
        };
        if let Err(e) = update.update(db).await {
            tracing::error!("Failed to update task log: {}", e);
        }
    }

    if let Err(e) = result {
        tracing::error!("{} failed: {}", name, e);
    }
}
