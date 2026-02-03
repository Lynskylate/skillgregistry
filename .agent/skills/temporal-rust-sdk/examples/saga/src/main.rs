use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use std::{sync::Arc, time::Duration};
use temporalio_client::{WorkflowClientTrait, WorkflowOptions};
use temporalio_common::worker::{WorkerConfig, WorkerTaskTypes, WorkerVersioningStrategy};
use temporalio_sdk::{
    ActContext, ActivityError, ActivityOptions, WfContext, WfExitValue, Worker, WorkflowResult,
};
use temporalio_sdk_core::init_worker;
use tracing::{error, info, Level};

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

const TASK_QUEUE: &str = "saga-q";
// const WORKFLOW_ID: &str = "saga-workflow-id";
const WORKFLOW_TYPE: &str = "saga-workflow";

// Activities
const ACT_RESERVE_CAR: &str = "reserve-car";
const ACT_CANCEL_CAR: &str = "cancel-car";
const ACT_RESERVE_HOTEL: &str = "reserve-hotel";
const ACT_CANCEL_HOTEL: &str = "cancel-hotel";
const ACT_RESERVE_FLIGHT: &str = "reserve-flight";
const ACT_CANCEL_FLIGHT: &str = "cancel-flight";

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
    info!("Starting Saga worker...");
    let namespace = examples_shared::temporal_namespace();
    let runtime = examples_shared::init_runtime()?;
    let client_options =
        examples_shared::build_client_options("rust-saga-worker", "0.1.0", "rust-saga-worker")?;
    let client = client_options.connect(&namespace, None).await?;

    let worker_config = WorkerConfig::builder()
        .namespace(namespace)
        .task_queue(TASK_QUEUE)
        .task_types(WorkerTaskTypes::all())
        .versioning_strategy(WorkerVersioningStrategy::None {
            build_id: "rust-saga".to_owned(),
        })
        .build()
        .map_err(|e| anyhow!(e))?;

    let core_worker = init_worker(&runtime, worker_config, client)?;
    let mut worker = Worker::new_from_core(Arc::new(core_worker), TASK_QUEUE);

    worker.register_wf(WORKFLOW_TYPE, saga_workflow);

    worker.register_activity(ACT_RESERVE_CAR, reserve_car);
    worker.register_activity(ACT_CANCEL_CAR, cancel_car);
    worker.register_activity(ACT_RESERVE_HOTEL, reserve_hotel);
    worker.register_activity(ACT_CANCEL_HOTEL, cancel_hotel);
    worker.register_activity(ACT_RESERVE_FLIGHT, reserve_flight);
    worker.register_activity(ACT_CANCEL_FLIGHT, cancel_flight);

    worker.run().await?;
    Ok(())
}

async fn run_starter() -> Result<()> {
    info!("Starting Saga workflow...");
    let namespace = examples_shared::temporal_namespace();
    let client_options =
        examples_shared::build_client_options("rust-saga-starter", "0.1.0", "rust-saga-starter")?;
    let client = client_options.connect(&namespace, None).await?;

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

    info!(
        "Saga Workflow started. Run ID: {}. Waiting for result...",
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

// --- Activities ---

async fn reserve_car(_ctx: ActContext, _: String) -> Result<String, ActivityError> {
    info!("Reserving car...");
    Ok("CarReservedID".to_string())
}
async fn cancel_car(_ctx: ActContext, _: String) -> Result<String, ActivityError> {
    info!("Cancelling car...");
    Ok("CarCancelled".to_string())
}

async fn reserve_hotel(_ctx: ActContext, _: String) -> Result<String, ActivityError> {
    info!("Reserving hotel...");
    Ok("HotelReservedID".to_string())
}
async fn cancel_hotel(_ctx: ActContext, _: String) -> Result<String, ActivityError> {
    info!("Cancelling hotel...");
    Ok("HotelCancelled".to_string())
}

async fn reserve_flight(_ctx: ActContext, _: String) -> Result<String, ActivityError> {
    info!("Reserving flight... (Simulating failure)");
    // Simulate failure to trigger saga compensation
    Err(ActivityError::NonRetryable(anyhow!(
        "Flight reservation system down"
    )))
}
async fn cancel_flight(_ctx: ActContext, _: String) -> Result<String, ActivityError> {
    info!("Cancelling flight...");
    Ok("FlightCancelled".to_string())
}

// --- Saga Logic ---

enum Compensation {
    Car,
    Hotel,
    Flight,
}

struct Saga {
    compensations: Vec<Compensation>,
}

impl Saga {
    fn new() -> Self {
        Self {
            compensations: vec![],
        }
    }

    fn add(&mut self, c: Compensation) {
        self.compensations.push(c);
    }

    async fn compensate(&mut self, ctx: &WfContext) {
        info!("Saga compensation triggered...");
        // Execute in reverse order
        while let Some(comp) = self.compensations.pop() {
            let act_type = match comp {
                Compensation::Car => ACT_CANCEL_CAR,
                Compensation::Hotel => ACT_CANCEL_HOTEL,
                Compensation::Flight => ACT_CANCEL_FLIGHT,
            };

            info!("Compensating: {}", act_type);

            let payload = match examples_shared::json_payload(&"dummy-input") {
                Ok(p) => p,
                Err(_) => return,
            };

            let opts = ActivityOptions {
                activity_type: act_type.to_string(),
                start_to_close_timeout: Some(Duration::from_secs(5)),
                task_queue: None,
                input: payload,
                ..Default::default()
            };

            // We ignore errors in compensation for this simple example, but in production you might retry
            let _ = ctx.activity(opts).await;
        }
        info!("Saga compensation finished.");
    }
}

async fn saga_workflow(ctx: WfContext) -> WorkflowResult<String> {
    info!("Saga Workflow started");
    let mut saga = Saga::new();

    // 1. Reserve Car
    if let Err(e) = execute_activity(&ctx, ACT_RESERVE_CAR).await {
        error!("Car reservation failed: {:?}", e);
        saga.compensate(&ctx).await;
        return Err(e);
    }
    saga.add(Compensation::Car);

    // 2. Reserve Hotel
    if let Err(e) = execute_activity(&ctx, ACT_RESERVE_HOTEL).await {
        error!("Hotel reservation failed: {:?}", e);
        saga.compensate(&ctx).await;
        return Err(e);
    }
    saga.add(Compensation::Hotel);

    // 3. Reserve Flight (Will fail)
    if let Err(e) = execute_activity(&ctx, ACT_RESERVE_FLIGHT).await {
        error!("Flight reservation failed (as expected): {:?}", e);
        saga.compensate(&ctx).await;
        return Ok(WfExitValue::Normal(
            "Flight failed, compensation executed.".to_string(),
        ));
    }
    saga.add(Compensation::Flight);

    Ok(WfExitValue::Normal(
        "Saga completed successfully.".to_string(),
    ))
}

async fn execute_activity(ctx: &WfContext, activity_type: &str) -> Result<(), anyhow::Error> {
    let payload = examples_shared::json_payload(&"dummy-input")?;

    let opts = ActivityOptions {
        activity_type: activity_type.to_string(),
        start_to_close_timeout: Some(Duration::from_secs(5)),
        task_queue: None,
        input: payload,
        ..Default::default()
    };

    let res = ctx.activity(opts).await;

    if let Some(status) = res.status {
        match status {
            temporalio_common::protos::coresdk::activity_result::activity_resolution::Status::Completed(_) => Ok(()),
            temporalio_common::protos::coresdk::activity_result::activity_resolution::Status::Failed(f) => {
                 Err(anyhow!("Activity failed: {:?}", f))
            }
             temporalio_common::protos::coresdk::activity_result::activity_resolution::Status::Cancelled(_) => {
                 Err(anyhow!("Activity cancelled"))
            }
            _ => Err(anyhow!("Activity failed with unknown status")),
        }
    } else {
        Err(anyhow!("Activity returned no status"))
    }
}
