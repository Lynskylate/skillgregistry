use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use std::{sync::Arc, time::Duration};
use temporalio_client::{WorkflowClientTrait, WorkflowOptions};
use temporalio_common::worker::{WorkerConfig, WorkerTaskTypes, WorkerVersioningStrategy};
use temporalio_sdk::{
    ActContext, ActivityError, ActivityOptions, CancellableFuture, WfContext, WfExitValue, Worker,
    WorkflowResult,
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
    Starter,
}

const TASK_QUEUE: &str = "batch-sliding-window-q";
// const WORKFLOW_ID: &str = "batch-sliding-window-workflow-id";
const WORKFLOW_TYPE: &str = "batch-sliding-window-workflow";
const ACTIVITY_TYPE: &str = "process-batch-activity";
const SIGNAL_NAME: &str = "add-item";

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    let cli = Cli::parse();
    match cli.command {
        Commands::Worker => run_worker().await,
        Commands::Starter => run_starter().await,
    }
}

async fn run_worker() -> Result<()> {
    info!("Starting worker...");
    let namespace = examples_shared::temporal_namespace();
    let runtime = examples_shared::init_runtime()?;
    let client_options =
        examples_shared::build_client_options("rust-batch-worker", "0.1.0", "rust-batch-worker")?;
    let client = client_options.connect(&namespace, None).await?;

    let worker_config = WorkerConfig::builder()
        .namespace(namespace)
        .task_queue(TASK_QUEUE)
        .task_types(WorkerTaskTypes::all())
        .versioning_strategy(WorkerVersioningStrategy::None {
            build_id: "rust-example".to_owned(),
        })
        .build()
        .map_err(|e| anyhow!(e))?;

    let core_worker = init_worker(&runtime, worker_config, client)?;
    let mut worker = Worker::new_from_core(Arc::new(core_worker), TASK_QUEUE);

    worker.register_activity(ACTIVITY_TYPE, process_batch);
    worker.register_wf(WORKFLOW_TYPE, batch_workflow);

    worker.run().await?;
    Ok(())
}

async fn run_starter() -> Result<()> {
    info!("Starting workflow...");
    let namespace = examples_shared::temporal_namespace();
    let client_options =
        examples_shared::build_client_options("rust-batch-starter", "0.1.0", "rust-batch-starter")?;
    let client = client_options.connect(&namespace, None).await?;

    // Start Workflow
    let wf_id = uuid::Uuid::new_v4().to_string();
    let res = client.start_workflow(
        vec![],
        TASK_QUEUE.to_string(),
        wf_id.clone(),
        WORKFLOW_TYPE.to_string(),
        None,
        WorkflowOptions {
            id_reuse_policy: temporalio_common::protos::temporal::api::enums::v1::WorkflowIdReusePolicy::AllowDuplicate,
            ..Default::default()
        },
    ).await?;
    info!("Workflow started. Sending signals...");

    // Send signals
    for i in 0..25 {
        let item = format!("Item {}", i);
        let payload = examples_shared::json_payload(&item)?;

        client
            .signal_workflow_execution(
                wf_id.clone(),
                "".to_string(),
                SIGNAL_NAME.to_string(),
                Some(
                    temporalio_common::protos::temporal::api::common::v1::Payloads {
                        payloads: vec![payload],
                    },
                ),
                None,
            )
            .await?;
        info!("Sent signal: {}", item);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Send finish signal
    client
        .signal_workflow_execution(
            wf_id.clone(),
            "".to_string(),
            "finish".to_string(),
            None,
            None,
        )
        .await?;
    info!("Sent finish signal. Waiting for result...");

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

async fn process_batch(_ctx: ActContext, items: Vec<String>) -> Result<String, ActivityError> {
    info!("Processing batch of {} items: {:?}", items.len(), items);
    Ok(format!("Processed {} items", items.len()))
}

async fn batch_workflow(ctx: WfContext) -> WorkflowResult<String> {
    let mut items: Vec<String> = Vec::new();
    let batch_size = 10;
    let timeout = Duration::from_secs(5);

    let mut signal_chan = ctx.make_signal_channel(SIGNAL_NAME);
    let mut finish_chan = ctx.make_signal_channel("finish");

    info!("Batch workflow started, waiting for signals...");

    loop {
        let mut timer = ctx.timer(timeout);
        let signal_recv = signal_chan.next();
        let finish_recv = finish_chan.next();

        // Wait for either a signal or a timer
        tokio::select! {
            // Finish signal
            Some(_) = finish_recv => {
                if !items.is_empty() {
                    info!("Finish signal received. Processing remaining {} items...", items.len());
                    execute_batch(&ctx, &mut items).await?;
                }
                return Ok(WfExitValue::Normal("Batch processing finished.".to_string()));
            }
            // Signal received
            Some(signal) = signal_recv => {
                // Cancel timer since we woke up
                timer.cancel(&ctx);

                if let Some(payload) = signal.input.first() {
                     if let Ok(item) = examples_shared::from_json_payload::<String>(payload) {
                         items.push(item);
                     }
                }

                if items.len() >= batch_size {
                    info!("Batch full (size {}), processing...", items.len());
                    execute_batch(&ctx, &mut items).await?;
                }
            }
            // Timer fired
            _ = &mut timer => {
                 if !items.is_empty() {
                    info!("Timeout reached, processing {} items...", items.len());
                    execute_batch(&ctx, &mut items).await?;
                 }
            }
        }
    }
}

async fn execute_batch(ctx: &WfContext, items: &mut Vec<String>) -> Result<(), anyhow::Error> {
    let payload = examples_shared::json_payload(items)?;

    // Clear items immediately
    items.clear();

    let activity_opts = ActivityOptions {
        activity_type: ACTIVITY_TYPE.to_string(),
        start_to_close_timeout: Some(Duration::from_secs(5)),
        task_queue: None,
        input: payload,
        ..Default::default()
    };

    let res = ctx.activity(activity_opts).await;

    if let Some(status) = res.status {
        match status {
            temporalio_common::protos::coresdk::activity_result::activity_resolution::Status::Completed(_) => {
                info!("Batch processing completed successfully");
            }
            _ => {
                return Err(anyhow!("Activity failed"));
            }
         }
    }
    Ok(())
}
