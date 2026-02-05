#![allow(dead_code)]
mod activities;
mod github;
#[cfg(test)]
mod index_flow_tests;
mod ports;
mod sync;
mod workflows;

use common::build_all;
use common::settings::Settings;
use common::{Repositories, Services};
use sea_orm::DatabaseConnection;
use std::str::FromStr;
use std::sync::Arc;
use temporalio_client::ClientOptions;
use temporalio_common::worker::{WorkerConfig, WorkerTaskTypes, WorkerVersioningStrategy};
use temporalio_sdk::Worker;
use temporalio_sdk_core::{init_worker, CoreRuntime, RuntimeOptions, Url};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
pub struct WorkerContext {
    pub db: Arc<DatabaseConnection>,
    pub repos: Repositories,
    pub services: Services,
    pub github: Arc<github::GithubClient>,
    pub settings: Arc<Settings>,
}
impl std::fmt::Debug for WorkerContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerContext")
            .field("db", &self.db)
            .field("settings", &self.settings)
            .field("s3", &"S3Service")
            .field("github", &"GithubClient")
            .finish()
    }
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

    let db = common::db::establish_connection(&db_url).await?;
    let db = Arc::new(db);

    let (repos, services) = build_all(db.clone(), &settings).await;

    let github = github::GithubClient::new(
        settings.github.token.clone(),
        settings.github.api_url.clone(),
    );

    let s3_bucket = settings.s3.bucket.clone();
    let s3_region = settings.s3.region.clone();
    let s3_endpoint = settings.s3.endpoint.clone();

    tracing::info!(
        s3_bucket = %s3_bucket,
        s3_region = %s3_region,
        s3_endpoint = %s3_endpoint.clone().unwrap_or_else(|| "<aws-default>".to_string()),
        "S3 config loaded"
    );

    let github = Arc::new(github);
    let settings = Arc::new(settings);

    let ctx = Arc::new(WorkerContext {
        db: Arc::clone(&db),
        repos,
        services,
        github: Arc::clone(&github),
        settings: Arc::clone(&settings),
    });

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

    // Register Activities with closures capturing WorkerContext
    let ctx_clone = Arc::clone(&ctx);
    worker.register_activity("discovery_activity", move |_ctx, queries| {
        let ctx = Arc::clone(&ctx_clone);
        async move { activities::discovery::discovery_activity_with_ctx(&ctx, queries).await }
    });

    let ctx_clone = Arc::clone(&ctx);
    worker.register_activity("fetch_pending_skills_activity", move |_ctx, _input| {
        let ctx = Arc::clone(&ctx_clone);
        async move { activities::sync::fetch_pending_skills_activity_with_ctx(&ctx, _input).await }
    });

    let ctx_clone = Arc::clone(&ctx);
    worker.register_activity("sync_single_skill_activity", move |_ctx, registry_id| {
        let ctx = Arc::clone(&ctx_clone);
        async move {
            activities::sync::sync_single_skill_activity_with_ctx(&ctx, registry_id).await
        }
    });

    let ctx_clone = Arc::clone(&ctx);
    worker.register_activity("fetch_repo_snapshot_activity", move |_ctx, registry_id| {
        let ctx = Arc::clone(&ctx_clone);
        async move {
            activities::sync::fetch_repo_snapshot_activity_with_ctx(&ctx, registry_id).await
        }
    });

    let ctx_clone = Arc::clone(&ctx);
    worker.register_activity(
        "apply_sync_from_snapshot_activity",
        move |_ctx, snapshot| {
            let ctx = Arc::clone(&ctx_clone);
            async move {
                activities::sync::apply_sync_from_snapshot_activity_with_ctx(&ctx, snapshot).await
            }
        },
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
