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

fn decode_activity_result<T: serde::de::DeserializeOwned>(
    status: Option<Status>,
    backoff_message: &str,
) -> Result<T> {
    match status {
        Some(Status::Completed(success)) => {
            let payload = success
                .result
                .ok_or_else(|| anyhow!("Activity completed but returned no result"))?;
            serde_json::from_slice(&payload.data)
                .map_err(|e| anyhow!("Failed to deserialize result: {}", e))
        }
        Some(Status::Failed(f)) => Err(anyhow!("Activity failed: {:?}", f)),
        Some(Status::Cancelled(_)) => Err(anyhow!("Activity cancelled")),
        Some(Status::Backoff(_)) => Err(anyhow!(backoff_message.to_string())),
        None => Err(anyhow!("Activity returned no status")),
    }
}

fn map_batch_status(status: Option<Status>) -> BatchActivityResult {
    match status {
        Some(Status::Completed(success)) => BatchActivityResult::Completed {
            payload: success.result.map(|payload| payload.data),
        },
        Some(Status::Failed(failure)) => BatchActivityResult::Failed(format!("{:?}", failure)),
        Some(Status::Cancelled(_)) => BatchActivityResult::Cancelled,
        Some(Status::Backoff(_)) => BatchActivityResult::Backoff,
        None => BatchActivityResult::MissingStatus,
    }
}

pub async fn execute_activity<T: serde::de::DeserializeOwned>(
    ctx: &WfContext,
    opts: ActivityOptions,
) -> Result<T> {
    let res = ctx.activity(opts).await;
    decode_activity_result(
        res.status,
        "Activity returned Backoff status; this helper expects Completed/Failed/Cancelled",
    )
}

pub async fn execute_local_activity<T: serde::de::DeserializeOwned>(
    ctx: &WfContext,
    opts: LocalActivityOptions,
) -> Result<T> {
    let res = ctx.local_activity(opts).await;
    decode_activity_result(
        res.status,
        "Local activity returned Backoff status; this helper expects Completed/Failed/Cancelled",
    )
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
            outcomes.push(map_batch_status(result.status));
        }
    }

    outcomes
}

#[cfg(test)]
mod tests {
    use super::*;
    use temporalio_common::protos::coresdk::activity_result::{
        activity_resolution::Status, Cancellation, DoBackoff, Failure, Success,
    };

    fn completed_status(payload: Option<Vec<u8>>) -> Status {
        Status::Completed(Success {
            result: payload.map(|data| {
                temporalio_common::protos::temporal::api::common::v1::Payload {
                    metadata: std::collections::HashMap::new(),
                    data,
                    ..Default::default()
                }
            }),
        })
    }

    #[test]
    fn create_json_payload_sets_encoding_and_body() {
        let payload = create_json_payload(&serde_json::json!({"id": 1}));
        assert_eq!(
            payload.metadata.get("encoding"),
            Some(&b"json/plain".to_vec())
        );
        let decoded: serde_json::Value = serde_json::from_slice(&payload.data).unwrap();
        assert_eq!(decoded["id"], 1);
    }

    #[test]
    fn decode_activity_result_covers_all_status_variants() {
        let ok: i32 = decode_activity_result(
            Some(completed_status(Some(serde_json::to_vec(&7).unwrap()))),
            "backoff",
        )
        .unwrap();
        assert_eq!(ok, 7);

        let no_payload = decode_activity_result::<i32>(Some(completed_status(None)), "backoff");
        assert!(no_payload.unwrap_err().to_string().contains("no result"));

        let bad_json = decode_activity_result::<i32>(
            Some(completed_status(Some(b"not-json".to_vec()))),
            "backoff",
        );
        assert!(bad_json.unwrap_err().to_string().contains("deserialize"));

        let failed =
            decode_activity_result::<i32>(Some(Status::Failed(Failure::default())), "backoff");
        assert!(failed.unwrap_err().to_string().contains("Activity failed"));

        let cancelled = decode_activity_result::<i32>(
            Some(Status::Cancelled(Cancellation::default())),
            "backoff",
        );
        assert!(cancelled.unwrap_err().to_string().contains("cancelled"));

        let backoff = decode_activity_result::<i32>(
            Some(Status::Backoff(DoBackoff::default())),
            "custom backoff",
        );
        assert!(backoff.unwrap_err().to_string().contains("custom backoff"));

        let missing = decode_activity_result::<i32>(None, "backoff");
        assert!(missing.unwrap_err().to_string().contains("no status"));
    }

    #[test]
    fn map_batch_status_maps_each_variant() {
        let completed = map_batch_status(Some(completed_status(Some(vec![1, 2, 3]))));
        match completed {
            BatchActivityResult::Completed { payload } => {
                assert_eq!(payload, Some(vec![1, 2, 3]));
            }
            _ => panic!("expected completed status"),
        }

        let failed = map_batch_status(Some(Status::Failed(Failure::default())));
        assert!(matches!(failed, BatchActivityResult::Failed(_)));

        assert!(matches!(
            map_batch_status(Some(Status::Cancelled(Cancellation::default()))),
            BatchActivityResult::Cancelled
        ));
        assert!(matches!(
            map_batch_status(Some(Status::Backoff(DoBackoff::default()))),
            BatchActivityResult::Backoff
        ));
        assert!(matches!(
            map_batch_status(None),
            BatchActivityResult::MissingStatus
        ));
    }
}
