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
use tracing::{info, error, Level};

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
const WORKFLOW_ID: &str = "saga-workflow-id";
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
    let server_options = ClientOptions::builder()
        .target_url(Url::from_str("http://localhost:7233")?)
        .client_name("rust-saga-worker")
        .client_version("0.1.0")
        .identity("rust-saga-worker".to_string())
        .build();
    let runtime = CoreRuntime::new_assume_tokio(RuntimeOptions::builder().telemetry_options(TelemetryOptions::builder().build()).build().map_err(|e| anyhow!(e))?).map_err(|e| anyhow!(e))?;
    let client = server_options.connect("default", None).await?;

    let worker_config = WorkerConfig::builder()
        .namespace("default")
        .task_queue(TASK_QUEUE)
        .task_types(WorkerTaskTypes::all())
        .versioning_strategy(WorkerVersioningStrategy::None { build_id: "rust-saga".to_owned() })
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
    let client_options = ClientOptions::builder()
        .target_url(Url::from_str("http://localhost:7233")?)
        .client_name("rust-saga-starter")
        .client_version("0.1.0")
        .identity("rust-saga-starter".to_string())
        .build();
    let client = client_options.connect("default", None).await?;

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
    
    info!("Saga Workflow started. Run ID: {}. Waiting for result...", res.run_id);

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
    Err(ActivityError::NonRetryable(anyhow!("Flight reservation system down")))
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
        Self { compensations: vec![] }
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
            
            let payload = temporalio_common::protos::temporal::api::common::v1::Payload {
                metadata: std::collections::HashMap::from([(
                    "encoding".to_string(), 
                    "json/plain".as_bytes().to_vec()
                )]),
                data: format!("\"{}\"", "dummy-input").into_bytes(),
                ..Default::default()
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
        return Ok(WfExitValue::Normal("Flight failed, compensation executed.".to_string()));
    }
    saga.add(Compensation::Flight);

    Ok(WfExitValue::Normal("Saga completed successfully.".to_string()))
}

async fn execute_activity(ctx: &WfContext, activity_type: &str) -> Result<(), anyhow::Error> {
    let payload = temporalio_common::protos::temporal::api::common::v1::Payload {
        metadata: std::collections::HashMap::from([(
            "encoding".to_string(), 
            "json/plain".as_bytes().to_vec()
        )]),
        data: format!("\"{}\"", "dummy-input").into_bytes(),
        ..Default::default()
    };

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
