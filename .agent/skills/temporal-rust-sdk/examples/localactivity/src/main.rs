use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use std::sync::Arc;
use std::time::Duration;
use temporalio_client::{WorkflowClientTrait, WorkflowOptions};
use temporalio_common::worker::{WorkerConfig, WorkerTaskTypes, WorkerVersioningStrategy};
use temporalio_sdk::{
    ActContext, ActivityError, ActivityOptions, LocalActivityOptions, WfContext, WfExitValue,
    Worker, WorkflowResult,
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
    Worker,
    Starter {
        #[arg(short, long, default_value = "Temporal")]
        name: String,
    },
}

const TASK_QUEUE: &str = "local-activity-q";
const WORKFLOW_TYPE: &str = "local-activity-workflow";
const LOCAL_ACTIVITY_TYPE: &str = "sanitize-name";
const ACTIVITY_TYPE: &str = "greet";

#[tokio::main]
async fn main() -> Result<()> {
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
    let client_options = examples_shared::build_client_options(
        "rust-localactivity-worker",
        "0.1.0",
        "rust-localactivity-worker",
    )?;
    let client = client_options.connect(&namespace, None).await?;

    let worker_config = WorkerConfig::builder()
        .namespace(namespace)
        .task_queue(TASK_QUEUE)
        .task_types(WorkerTaskTypes::all())
        .versioning_strategy(WorkerVersioningStrategy::None {
            build_id: "rust-localactivity".to_owned(),
        })
        .build()
        .map_err(|e| anyhow!(e))?;

    let core_worker = init_worker(&runtime, worker_config, client)?;
    let mut worker = Worker::new_from_core(Arc::new(core_worker), TASK_QUEUE);

    worker.register_wf(WORKFLOW_TYPE, local_activity_workflow);
    worker.register_activity(LOCAL_ACTIVITY_TYPE, sanitize_name);
    worker.register_activity(ACTIVITY_TYPE, greet);

    worker.run().await?;
    Ok(())
}

async fn run_starter(name: String) -> Result<()> {
    info!("Starting workflow with name: {}", name);

    let namespace = examples_shared::temporal_namespace();
    let client_options = examples_shared::build_client_options(
        "rust-localactivity-starter",
        "0.1.0",
        "rust-localactivity-starter",
    )?;
    let client = client_options.connect(&namespace, None).await?;

    let payload = examples_shared::json_payload(&name)?;
    let wf_id = uuid::Uuid::new_v4().to_string();

    let res = client
        .start_workflow(
            vec![payload],
            TASK_QUEUE.to_string(),
            wf_id.clone(),
            WORKFLOW_TYPE.to_string(),
            None,
            WorkflowOptions {
                id_reuse_policy:
                    temporalio_common::protos::temporal::api::enums::v1::WorkflowIdReusePolicy::AllowDuplicate,
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

async fn sanitize_name(_ctx: ActContext, input: String) -> Result<String, ActivityError> {
    info!("Local Activity: sanitize-name input={}", input);
    let trimmed = input.trim().to_string();
    Ok(trimmed)
}

async fn greet(_ctx: ActContext, name: String) -> Result<String, ActivityError> {
    info!("Activity: greet name={}", name);
    Ok(format!("Hello, {}!", name))
}

async fn local_activity_workflow(ctx: WfContext) -> WorkflowResult<String> {
    let input_args = ctx.get_args();
    let name: String = if let Some(payload) = input_args.first() {
        examples_shared::from_json_payload(payload).unwrap_or_else(|_| "Stranger".to_string())
    } else {
        "Stranger".to_string()
    };

    let local_opts = LocalActivityOptions {
        activity_type: LOCAL_ACTIVITY_TYPE.to_string(),
        start_to_close_timeout: Some(Duration::from_secs(2)),
        input: examples_shared::json_payload(&name).map_err(|e| anyhow!(e))?,
        ..Default::default()
    };

    let local_res = ctx.local_activity(local_opts).await;
    let mut sanitized = name;
    if let Some(status) = local_res.status {
        if let temporalio_common::protos::coresdk::activity_result::activity_resolution::Status::Completed(success) =
            status
        {
            if let Some(payload) = success.result {
                sanitized = examples_shared::from_json_payload(&payload).unwrap_or_else(|_| sanitized);
            }
        }
    }

    let greet_opts = ActivityOptions {
        activity_type: ACTIVITY_TYPE.to_string(),
        start_to_close_timeout: Some(Duration::from_secs(5)),
        task_queue: None,
        input: examples_shared::json_payload(&sanitized).map_err(|e| anyhow!(e))?,
        ..Default::default()
    };

    let greet_res = ctx.activity(greet_opts).await;
    if let Some(status) = greet_res.status {
        match status {
            temporalio_common::protos::coresdk::activity_result::activity_resolution::Status::Completed(success) => {
                if let Some(payload) = success.result {
                    let result_str: String = examples_shared::from_json_payload(&payload).unwrap_or_default();
                    return Ok(WfExitValue::Normal(result_str));
                }
                return Ok(WfExitValue::Normal("Activity completed (no result).".to_string()));
            }
            _ => return Err(anyhow!("Activity failed")),
        }
    }

    Ok(WfExitValue::Normal(
        "Activity finished but no status?".to_string(),
    ))
}
