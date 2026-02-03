pub mod discovery_workflow;
pub mod sync_repo_workflow;
pub mod sync_scheduler_workflow;

use anyhow::{anyhow, Result};
use temporalio_common::protos::coresdk::activity_result::activity_resolution::Status;
use temporalio_sdk::{ActivityOptions, WfContext};

pub fn create_json_payload(
    data: &impl serde::Serialize,
) -> temporalio_common::protos::temporal::api::common::v1::Payload {
    temporalio_common::protos::temporal::api::common::v1::Payload {
        metadata: std::collections::HashMap::from([(
            "encoding".to_string(),
            "json/plain".as_bytes().to_vec(),
        )]),
        data: serde_json::to_vec(data).unwrap_or_default(),
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
                // If T is unit, return default?
                // Hack: try to deserialize empty bytes or just error
                // For now error if result missing but T expected
                Err(anyhow!("Activity completed but returned no result"))
            }
            Status::Failed(f) => Err(anyhow!("Activity failed: {:?}", f)),
            Status::Cancelled(_) => Err(anyhow!("Activity cancelled")),
            Status::Backoff(_) => Err(anyhow!("Activity backoff?")), // Should not happen in result
        }
    } else {
        Err(anyhow!("Activity returned no status"))
    }
}
