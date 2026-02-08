use anyhow::Result;
use common::settings::Settings;
use std::str::FromStr;
use temporalio_client::{ClientOptions, WorkflowClientTrait, WorkflowOptions};
use temporalio_sdk_core::Url;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let settings = Settings::new()?;
    let server_url = settings.temporal.server_url;
    let task_queue = settings.temporal.task_queue;

    let client_options = ClientOptions::builder()
        .target_url(Url::from_str(&server_url)?)
        .client_name("skill-starter")
        .client_version("0.1.0")
        .identity("skill-starter".to_string())
        .build();

    let client = client_options.connect("default", None).await?;

    // 1. Start Discovery Workflow
    let discovery_id = format!("discovery-{}", uuid::Uuid::new_v4());
    let opts = WorkflowOptions {
        id_reuse_policy: temporalio_common::protos::temporal::api::enums::v1::WorkflowIdReusePolicy::AllowDuplicate,
        ..Default::default()
    };

    tracing::info!("Starting Discovery Workflow: {}", discovery_id);
    client
        .start_workflow(
            vec![],
            task_queue.clone(),
            discovery_id.clone(),
            "discovery_workflow".to_string(),
            None,
            opts.clone(),
        )
        .await?;

    // 2. Start Sync Scheduler Workflow
    let sync_id = format!("sync-sched-{}", uuid::Uuid::new_v4());

    tracing::info!("Starting Sync Scheduler Workflow: {}", sync_id);
    client
        .start_workflow(
            vec![],
            task_queue,
            sync_id.clone(),
            "sync_scheduler_workflow".to_string(),
            None,
            opts,
        )
        .await?;

    tracing::info!("Workflows started successfully.");
    Ok(())
}
