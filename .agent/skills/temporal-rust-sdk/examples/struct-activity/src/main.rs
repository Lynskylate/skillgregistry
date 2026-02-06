use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use temporalio_client::{WorkflowClientTrait, WorkflowOptions};
use temporalio_common::worker::{WorkerConfig, WorkerTaskTypes, WorkerVersioningStrategy};
use temporalio_sdk::{
    ActContext, ActivityError, ActivityOptions, WfContext, WfExitValue, Worker, WorkflowResult,
};
use temporalio_sdk_core::init_worker;
use tracing::{info, Level};

#[derive(Parser)]
#[command(author, version, about = "Struct method Activity example", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Worker,
    Starter {
        #[arg(short, long, default_value = "World")]
        name: String,
    },
}

const TASK_QUEUE: &str = "struct-activity-q";
const WORKFLOW_TYPE: &str = "struct-activity-workflow";
const ACTIVITY_TYPE: &str = "greet-activity";

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GreetInput {
    name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GreetOutput {
    message: String,
}

#[derive(Clone)]
struct GreeterService {
    prefix: String,
}

impl GreeterService {
    async fn greet(&self, input: GreetInput) -> Result<GreetOutput, ActivityError> {
        let msg = format!("{} {}", self.prefix, input.name);
        Ok(GreetOutput { message: msg })
    }
}

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
    let client_options =
        examples_shared::build_client_options("rust-worker", "0.1.0", "struct-activity-worker")?;
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

    let service = Arc::new(GreeterService {
        prefix: "Hello,".to_string(),
    });
    let svc_clone = Arc::clone(&service);
    worker.register_activity(ACTIVITY_TYPE, move |_ctx: ActContext, input: GreetInput| {
        let svc = Arc::clone(&svc_clone);
        async move { svc.greet(input).await }
    });

    worker.register_wf(WORKFLOW_TYPE, struct_activity_workflow);

    info!("Worker started. Press Ctrl+C to stop.");
    worker.run().await?;
    Ok(())
}

async fn run_starter(name: String) -> Result<()> {
    info!("Starting workflow with name: {}", name);

    let namespace = examples_shared::temporal_namespace();
    let client_options =
        examples_shared::build_client_options("rust-starter", "0.1.0", "struct-activity-starter")?;
    let client = client_options.connect(&namespace, None).await?;

    let payload = examples_shared::json_payload(&GreetInput { name })?;
    let wf_id = uuid::Uuid::new_v4().to_string();
    let res = client
        .start_workflow(
            vec![payload],
            TASK_QUEUE.to_string(),
            wf_id.clone(),
            WORKFLOW_TYPE.to_string(),
            None,
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

async fn struct_activity_workflow(ctx: WfContext) -> WorkflowResult<String> {
    info!("Workflow started");

    let input_args = ctx.get_args();
    let greet_input: GreetInput = if let Some(payload) = input_args.first() {
        examples_shared::from_json_payload(payload)
            .unwrap_or_else(|_| GreetInput { name: "Stranger".into() })
    } else {
        GreetInput { name: "Stranger".into() }
    };

    let activity_opts = ActivityOptions {
        activity_type: ACTIVITY_TYPE.to_string(),
        start_to_close_timeout: Some(Duration::from_secs(10)),
        input: examples_shared::json_payload(&greet_input).map_err(|e| anyhow!(e))?,
        ..Default::default()
    };
    let res = ctx.activity(activity_opts).await;

    if let Some(status) = res.status {
        match status {
            temporalio_common::protos::coresdk::activity_result::activity_resolution::Status::Completed(success) => {
                if let Some(payload) = success.result {
                    let out: GreetOutput = serde_json::from_slice(&payload.data).unwrap_or(GreetOutput { message: "No message".into() });
                    return Ok(WfExitValue::Normal(out.message));
                }
            }
            _ => return Err(anyhow!("Activity failed or cancelled")),
        }
    }

    Ok(WfExitValue::Normal("Activity finished but no result?".to_string()))
}
