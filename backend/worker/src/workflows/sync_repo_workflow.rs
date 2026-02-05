use crate::activities::sync::{SnapshotResult, SyncResult};
use crate::workflows::{create_json_payload, execute_local_activity};
use std::time::Duration;
use temporalio_common::protos::temporal::api::common::v1::RetryPolicy;
use temporalio_sdk::{LocalActivityOptions, WfContext, WfExitValue};

pub async fn sync_repo_workflow(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    // Get input (registry_id)
    let args = ctx.get_args();
    let registry_id: i32 = if let Some(payload) = args.first() {
        serde_json::from_slice(&payload.data).unwrap_or(0)
    } else {
        return Ok(WfExitValue::Normal("Missing registry_id input".to_string()));
    };

    if registry_id == 0 {
        return Ok(WfExitValue::Normal("Invalid registry_id".to_string()));
    }

    let retry_policy = RetryPolicy {
        maximum_attempts: 5,
        ..Default::default()
    };

    let snapshot_opts = LocalActivityOptions {
        activity_type: "fetch_repo_snapshot_activity".to_string(),
        input: create_json_payload(&registry_id),
        start_to_close_timeout: Some(Duration::from_secs(120)),
        schedule_to_close_timeout: Some(Duration::from_secs(180)),
        retry_policy: retry_policy.clone(),
        timer_backoff_threshold: Some(Duration::from_secs(5)),
        ..Default::default()
    };

    let snapshot_res: SnapshotResult = match execute_local_activity(&ctx, snapshot_opts).await {
        Ok(res) => res,
        Err(e) => return Ok(WfExitValue::Normal(format!("Fetch Snapshot Failed: {}", e))),
    };

    let snapshot = match snapshot_res {
        SnapshotResult::Skipped { status } => {
            return Ok(WfExitValue::Normal(format!("Sync Skipped: {}", status)))
        }
        SnapshotResult::Snapshot(s) => s,
    };

    let apply_opts = LocalActivityOptions {
        activity_type: "apply_sync_from_snapshot_activity".to_string(),
        input: create_json_payload(&snapshot),
        start_to_close_timeout: Some(Duration::from_secs(600)),
        schedule_to_close_timeout: Some(Duration::from_secs(900)),
        retry_policy,
        timer_backoff_threshold: Some(Duration::from_secs(5)),
        ..Default::default()
    };

    let res: SyncResult = match execute_local_activity(&ctx, apply_opts).await {
        Ok(res) => res,
        Err(e) => return Ok(WfExitValue::Normal(format!("Sync Failed: {}", e))),
    };

    Ok(WfExitValue::Normal(format!("Sync Completed: {:?}", res)))
}
