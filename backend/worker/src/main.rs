#![allow(dead_code)]
mod activities;
mod github;
mod ports;
mod workflows;

use common::config::AppConfig;
use common::db;
use common::s3::S3Service;
use sea_orm::DatabaseConnection;
use std::str::FromStr;
use std::sync::Arc;
use temporalio_client::ClientOptions;
use temporalio_common::worker::{WorkerConfig, WorkerTaskTypes, WorkerVersioningStrategy};
use temporalio_sdk::Worker;
use temporalio_sdk_core::{init_worker, CoreRuntime, RuntimeOptions, Url};
use tokio::sync::OnceCell;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct AppState {
    pub db: DatabaseConnection,
    pub s3: S3Service,
    pub github: github::GithubClient,
    pub config: Arc<AppConfig>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("db", &self.db)
            .field("config", &self.config)
            .field("s3", &"S3Service")
            .field("github", &"GithubClient")
            .finish()
    }
}

pub static APP_STATE: OnceCell<AppState> = OnceCell::const_new();

pub async fn get_app_state() -> &'static AppState {
    APP_STATE.get().expect("AppState not initialized")
}

fn get_host_name() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "Unknown".to_string())
}

fn get_worker_identity(task_queue: &str) -> String {
    format!("{}@{}@{}", std::process::id(), get_host_name(), task_queue)
}

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

    let db_url = std::env::var("DATABASE_URL")
        .ok()
        .or(config.database.as_ref().and_then(|d| d.url.clone()))
        .unwrap_or_else(|| "sqlite://skillregistry.db?mode=rwc".to_string());

    let db = db::establish_connection(&db_url).await?;

    let s3_bucket = std::env::var("S3_BUCKET")
        .or_else(|_| std::env::var("S3_BUCKET_NAME"))
        .unwrap_or_else(|_| config.s3.bucket.clone());

    let s3_endpoint = std::env::var("S3_ENDPOINT")
        .ok()
        .or_else(|| std::env::var("S3_ENDPOINT_URL").ok())
        .or_else(|| std::env::var("AWS_ENDPOINT_URL").ok())
        .or(config.s3.endpoint.clone());

    let inferred_region = s3_endpoint.as_deref().and_then(|ep| {
        let ep = ep.trim_matches('"');
        let ep = ep
            .strip_prefix("https://")
            .or_else(|| ep.strip_prefix("http://"))
            .unwrap_or(ep);
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
        .unwrap_or_else(|| config.s3.region.clone());

    tracing::info!(
        s3_bucket = %s3_bucket,
        s3_region = %s3_region,
        s3_endpoint = %s3_endpoint.clone().unwrap_or_else(|| "<aws-default>".to_string()),
        "S3 config loaded"
    );

    let s3 = S3Service::new(s3_bucket, s3_region, s3_endpoint).await;

    let github_token = std::env::var("GITHUB_TOKEN").ok();
    let github = github::GithubClient::new(github_token);

    // Initialize AppState
    let state = AppState {
        db,
        s3,
        github,
        config: Arc::new(config.clone()),
    };
    APP_STATE.set(state).expect("Failed to set AppState");

    // Temporal Setup
    let server_url = std::env::var("TEMPORAL_SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:7233".to_string());
    let task_queue = "skill-registry-queue";
    let worker_identity = get_worker_identity(task_queue);

    let server_options = ClientOptions::builder()
        .target_url(Url::from_str(&server_url)?)
        .client_name("skill-worker")
        .client_version("0.1.0")
        .identity(worker_identity)
        .build();

    let client = server_options.connect("default", None).await?;

    let runtime_options = RuntimeOptions::builder().build().unwrap();
    let runtime = CoreRuntime::new_assume_tokio(runtime_options).map_err(|e| anyhow::anyhow!(e))?;

    let worker_config = WorkerConfig::builder()
        .namespace("default")
        .task_queue(task_queue)
        .task_types(WorkerTaskTypes::all())
        .versioning_strategy(WorkerVersioningStrategy::None {
            build_id: "rust-worker-0.1.0".to_string(),
        })
        .build()
        .map_err(|e| anyhow::anyhow!(e))?;

    let core_worker = init_worker(&runtime, worker_config, client)?;
    let mut worker = Worker::new_from_core(Arc::new(core_worker), task_queue);

    // Register Activities
    worker.register_activity(
        "discovery_activity",
        activities::discovery::discovery_activity,
    );
    worker.register_activity(
        "fetch_pending_skills_activity",
        activities::sync::fetch_pending_skills_activity,
    );
    worker.register_activity(
        "sync_single_skill_activity",
        activities::sync::sync_single_skill_activity,
    );

    // Register Workflows
    worker.register_wf(
        "discovery_workflow",
        workflows::discovery_workflow::discovery_workflow,
    );
    worker.register_wf(
        "sync_scheduler_workflow",
        workflows::sync_scheduler_workflow::sync_scheduler_workflow,
    );
    worker.register_wf(
        "sync_repo_workflow",
        workflows::sync_repo_workflow::sync_repo_workflow,
    );

    tracing::info!("Starting Temporal Worker on queue '{}'...", task_queue);
    worker.run().await?;

    Ok(())
}
