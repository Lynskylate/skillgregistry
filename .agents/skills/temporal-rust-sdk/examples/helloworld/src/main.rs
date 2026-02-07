use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use std::{sync::Arc, time::Duration};
use temporalio_client::{WorkflowClientTrait, WorkflowOptions};
use temporalio_common::worker::{WorkerConfig, WorkerTaskTypes, WorkerVersioningStrategy};
use temporalio_sdk::{
    ActContext, ActivityError, ActivityOptions, WfContext, WfExitValue, Worker, WorkflowResult,
};
use temporalio_sdk_core::init_worker;
use tracing::{info, Level};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the worker
    Worker,
    /// Start the workflow
    Starter {
        #[arg(short, long, default_value = "World")]
        name: String,
    },
}

const TASK_QUEUE: &str = "hello-world-q";
// const WORKFLOW_ID: &str = "hello-world-workflow-id";
const WORKFLOW_TYPE: &str = "hello-world-workflow";
const ACTIVITY_TYPE: &str = "say-hello-activity";

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Worker => run_worker().await,
        Commands::Starter { name } => run_starter(name).await,
    }
}

async fn run_worker() -> Result<()> {
    info!("Starting worker...");

    let namespace = examples_shared::temporal_namespace();
    let runtime = examples_shared::init_runtime()?;
    let client_options =
        examples_shared::build_client_options("rust-worker", "0.1.0", "rust-worker")?;
    let client = client_options.connect(&namespace, None).await?;

    let worker_config = WorkerConfig::builder()
        .namespace(namespace)
        .task_queue(TASK_QUEUE)
        .task_types(WorkerTaskTypes::all())
        .versioning_strategy(WorkerVersioningStrategy::None {
            build_id: "rust-sdk-example".to_owned(),
        })
        .build()
        .map_err(|e| anyhow!(e))?;

    let core_worker = init_worker(&runtime, worker_config, client)?;

    let mut worker = Worker::new_from_core(Arc::new(core_worker), TASK_QUEUE);

    // Register Activity
    worker.register_activity(ACTIVITY_TYPE, say_hello);

    // Register Workflow
    worker.register_wf(WORKFLOW_TYPE, hello_world_workflow);

    info!("Worker started. Press Ctrl+C to stop.");
    worker.run().await?;

    Ok(())
}

async fn run_starter(name: String) -> Result<()> {
    info!("Starting workflow with name: {}", name);

    let namespace = examples_shared::temporal_namespace();
    let client_options =
        examples_shared::build_client_options("rust-starter", "0.1.0", "rust-starter")?;
    let client = client_options.connect(&namespace, None).await?;

    let payload = examples_shared::json_payload(&name)?;

    let wf_id = uuid::Uuid::new_v4().to_string();
    let res = client
        .start_workflow(
            vec![payload],
            TASK_QUEUE.to_string(),
            wf_id.clone(),
            WORKFLOW_TYPE.to_string(),
            None, // request_id
            WorkflowOptions {
                id_reuse_policy: temporalio_common::protos::temporal::api::enums::v1::WorkflowIdReusePolicy::AllowDuplicate,
                ..Default::default()
            },
        )
        .await?;

    info!(
        "Workflow started. Run ID: {}. Waiting for result...",
        res.run_id
    );

    let result = examples_shared::poll_workflow_result::<String, _>(
        &client,
        &wf_id,
        &res.run_id,
        Duration::from_millis(500),
    )
    .await?;

    if let Some(result) = result {
        info!("Workflow Result: {}", result);
    } else {
        info!("Workflow completed (no result).");
    }

    Ok(())
}

// Activity Definition
async fn say_hello(_ctx: ActContext, name: String) -> Result<String, ActivityError> {
    info!("Activity started, saying hello to {}", name);
    Ok(format!("Hello, {}!", name))
}

// Workflow Definition
async fn hello_world_workflow(ctx: WfContext) -> WorkflowResult<String> {
    info!("Workflow started");

    let input_args = ctx.get_args();
    let name: String = if let Some(payload) = input_args.first() {
        examples_shared::from_json_payload(payload).unwrap_or_else(|_| "Stranger".to_string())
    } else {
        "Stranger".to_string()
    };

    let activity_opts = ActivityOptions {
        activity_type: ACTIVITY_TYPE.to_string(),
        start_to_close_timeout: Some(Duration::from_secs(5)),
        task_queue: None, // defaults to current
        input: examples_shared::json_payload(&name).map_err(|e| anyhow!(e))?,
        ..Default::default()
    };

    // Schedule the activity
    let res = ctx.activity(activity_opts).await;

    // Check status
    if let Some(status) = res.status {
        match status {
            temporalio_common::protos::coresdk::activity_result::activity_resolution::Status::Completed(success) => {
                if let Some(payload) = success.result {
                     let result_str: String = serde_json::from_slice(&payload.data).unwrap_or_default();
                     return Ok(WfExitValue::Normal(result_str));
                }
            }
            _ => {
                return Err(anyhow!("Activity failed or cancelled"));
            }
         }
    }

    Ok(WfExitValue::Normal(
        "Activity finished but no result?".to_string(),
    ))
}
