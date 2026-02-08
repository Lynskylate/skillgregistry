pub mod discovery_workflow;
pub mod sync_repo_workflow;
pub mod sync_scheduler_workflow;
pub mod trigger_registry_workflow;

use anyhow::{anyhow, Result};
use futures::future::join_all;
use temporalio_common::protos::coresdk::activity_result::activity_resolution::Status;
use temporalio_sdk::{ActivityOptions, LocalActivityOptions, WfContext};

pub enum BatchActivityResult {
    Completed { payload: Option<Vec<u8>> },
    Failed(String),
    Cancelled,
    Backoff,
    MissingStatus,
}

pub fn create_json_payload(
    data: &impl serde::Serialize,
) -> temporalio_common::protos::temporal::api::common::v1::Payload {
    temporalio_common::protos::temporal::api::common::v1::Payload {
        metadata: std::collections::HashMap::from([(
            "encoding".to_string(),
            "json/plain".as_bytes().to_vec(),
        )]),
        data: serde_json::to_vec(data).expect("failed to serialize JSON payload"),
        ..Default::default()
    }
}

pub async fn execute_activity<T: serde::de::DeserializeOwned>(
    ctx: &WfContext,
    opts: ActivityOptions,
) -> Result<T> {
    let res = ctx.activity(opts).await;

    if let Some(status) = res.status {
        match status {
            Status::Completed(success) => {
                if let Some(payload) = success.result {
                    let result: T = serde_json::from_slice(&payload.data)
                        .map_err(|e| anyhow!("Failed to deserialize result: {}", e))?;
                    return Ok(result);
                }
                Err(anyhow!("Activity completed but returned no result"))
            }
            Status::Failed(f) => Err(anyhow!("Activity failed: {:?}", f)),
            Status::Cancelled(_) => Err(anyhow!("Activity cancelled")),
            Status::Backoff(_) => Err(anyhow!(
                "Activity returned Backoff status; this helper expects Completed/Failed/Cancelled"
            )),
        }
    } else {
        Err(anyhow!("Activity returned no status"))
    }
}

pub async fn execute_local_activity<T: serde::de::DeserializeOwned>(
    ctx: &WfContext,
    opts: LocalActivityOptions,
) -> Result<T> {
    let res = ctx.local_activity(opts).await;

    if let Some(status) = res.status {
        match status {
            Status::Completed(success) => {
                if let Some(payload) = success.result {
                    let result: T = serde_json::from_slice(&payload.data)
                        .map_err(|e| anyhow!("Failed to deserialize result: {}", e))?;
                    return Ok(result);
                }
                Err(anyhow!("Activity completed but returned no result"))
            }
            Status::Failed(f) => Err(anyhow!("Activity failed: {:?}", f)),
            Status::Cancelled(_) => Err(anyhow!("Activity cancelled")),
            Status::Backoff(_) => Err(anyhow!(
                "Local activity returned Backoff status; this helper expects Completed/Failed/Cancelled"
            )),
        }
    } else {
        Err(anyhow!("Activity returned no status"))
    }
}

pub async fn execute_activity_batch<F>(
    ctx: &WfContext,
    inputs: &[i32],
    chunk_size: usize,
    mut to_options: F,
) -> Vec<BatchActivityResult>
where
    F: FnMut(i32) -> ActivityOptions,
{
    let mut outcomes = Vec::new();

    for chunk in inputs.chunks(chunk_size) {
        let futures = chunk
            .iter()
            .map(|input| ctx.activity(to_options(*input)))
            .collect::<Vec<_>>();

        for result in join_all(futures).await {
            let outcome = match result.status {
                Some(Status::Completed(success)) => BatchActivityResult::Completed {
                    payload: success.result.map(|payload| payload.data),
                },
                Some(Status::Failed(failure)) => {
                    BatchActivityResult::Failed(format!("{:?}", failure))
                }
                Some(Status::Cancelled(_)) => BatchActivityResult::Cancelled,
                Some(Status::Backoff(_)) => BatchActivityResult::Backoff,
                None => BatchActivityResult::MissingStatus,
            };
            outcomes.push(outcome);
        }
    }

    outcomes
}
