use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use std::{str::FromStr, sync::Arc, time::Duration};
use temporalio_client::{ClientOptions, WorkflowClientTrait, WorkflowOptions};
use temporalio_common::{
    telemetry::TelemetryOptions,
    worker::{WorkerConfig, WorkerTaskTypes, WorkerVersioningStrategy},
};
use temporalio_sdk::{
    sdk_client_options, ActContext, ActivityError, ActivityOptions, WfContext, WfExitValue, Worker,
    WorkflowResult,
};
use temporalio_sdk_core::{init_worker, CoreRuntime, RuntimeOptions, Url};
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
const WORKFLOW_ID: &str = "hello-world-workflow-id";
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

    let server_options = ClientOptions::builder()
        .target_url(Url::from_str("http://localhost:7233")?)
        .client_name("rust-worker")
        .client_version("0.1.0")
        .identity("rust-worker".to_string())
        .build();
    
    let telemetry_options = TelemetryOptions::builder().build();
    let runtime_options = RuntimeOptions::builder()
        .telemetry_options(telemetry_options)
        .build()
        .map_err(|e| anyhow!(e))?;
    let runtime = CoreRuntime::new_assume_tokio(runtime_options).map_err(|e| anyhow!(e))?;

    let client = server_options.connect("default", None).await?;

    let worker_config = WorkerConfig::builder()
        .namespace("default")
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

    let client_options = ClientOptions::builder()
        .target_url(Url::from_str("http://localhost:7233")?)
        .client_name("rust-starter")
        .client_version("0.1.0")
        .identity("rust-starter".to_string())
        .build();
    let client = client_options.connect("default", None).await?;
    
    let payload = temporalio_common::protos::temporal::api::common::v1::Payload {
        metadata: std::collections::HashMap::from([(
            "encoding".to_string(), 
            "json/plain".as_bytes().to_vec()
        )]),
        data: format!("\"{}\"", name).into_bytes(),
        ..Default::default()
    };

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

    info!("Workflow started. Run ID: {}. Waiting for result...", res.run_id);

    // Poll for result
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        let history_response = client.get_workflow_execution_history(
            wf_id.clone(), 
            Some(res.run_id.clone()), 
            vec![]
        ).await?;

        if let Some(history) = history_response.history {
            for event in history.events {
                if event.event_type == temporalio_common::protos::temporal::api::enums::v1::EventType::WorkflowExecutionCompleted as i32 {
                    if let Some(temporalio_common::protos::temporal::api::history::v1::history_event::Attributes::WorkflowExecutionCompletedEventAttributes(attrs)) = event.attributes {
                        if let Some(payloads) = attrs.result {
                            if let Some(payload) = payloads.payloads.first() {
                                 let result: String = serde_json::from_slice(&payload.data).unwrap_or_default();
                                 info!("Workflow Result: {}", result);
                                 return Ok(());
                            }
                        }
                    }
                    info!("Workflow completed (no result).");
                    return Ok(());
                } else if event.event_type == temporalio_common::protos::temporal::api::enums::v1::EventType::WorkflowExecutionFailed as i32 {
                     return Err(anyhow!("Workflow failed"));
                }
            }
        }
    }
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
        // Simple manual JSON decoding for this example
        serde_json::from_slice(&payload.data).unwrap_or_else(|_| "Stranger".to_string())
    } else {
        "Stranger".to_string()
    };

    let activity_opts = ActivityOptions {
        activity_type: ACTIVITY_TYPE.to_string(),
        start_to_close_timeout: Some(Duration::from_secs(5)),
        task_queue: None, // defaults to current
        input: temporalio_common::protos::temporal::api::common::v1::Payload {
            metadata: std::collections::HashMap::from([("encoding".to_string(), "json/plain".as_bytes().to_vec())]),
            data: serde_json::to_vec(&name).unwrap(),
            ..Default::default()
        },
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

    Ok(WfExitValue::Normal("Activity finished but no result?".to_string()))
}
