#![allow(dead_code)]
mod activities;
mod github;
mod ports;
mod workflows;

use common::db;
use common::s3::S3Service;
use common::settings::Settings;
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
    pub settings: Arc<Settings>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("db", &self.db)
            .field("settings", &self.settings)
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
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "worker=debug,common=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let settings = Settings::new().expect("Failed to load configuration");

    let db_url = settings.database.url.clone();

    let db = db::establish_connection(&db_url).await?;

    let s3_bucket = settings.s3.bucket.clone();
    let s3_region = settings.s3.region.clone();
    let s3_endpoint = settings.s3.endpoint.clone();

    tracing::info!(
        s3_bucket = %s3_bucket,
        s3_region = %s3_region,
        s3_endpoint = %s3_endpoint.clone().unwrap_or_else(|| "<aws-default>".to_string()),
        "S3 config loaded"
    );

    let s3 = S3Service::new(
        s3_bucket,
        s3_region,
        s3_endpoint,
        settings.s3.access_key_id.clone(),
        settings.s3.secret_access_key.clone(),
        settings.s3.force_path_style,
    )
    .await;

    let github = github::GithubClient::new(
        settings.github.token.clone(),
        Some(settings.github.api_url.clone()),
    );

    // Initialize AppState
    let state = AppState {
        db,
        s3,
        github,
        settings: Arc::new(settings.clone()),
    };
    APP_STATE.set(state).expect("Failed to set AppState");

    // Temporal Setup
    let server_url = settings.temporal.server_url.clone();
    let task_queue = settings.temporal.task_queue.as_str();
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
