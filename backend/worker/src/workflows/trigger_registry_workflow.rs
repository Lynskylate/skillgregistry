use crate::activities::discovery::DiscoveryResult;
use crate::workflows::{create_json_payload, execute_activity};
use futures::future::join_all;
use std::time::Duration;
use temporalio_common::protos::coresdk::activity_result::activity_resolution::Status;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

pub async fn trigger_registry_workflow(
    ctx: WfContext,
) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let registry_id: i32 = if let Some(payload) = args.first() {
        serde_json::from_slice(&payload.data).unwrap_or(0)
    } else {
        return Ok(WfExitValue::Normal("Missing registry_id input".to_string()));
    };

    if registry_id <= 0 {
        return Ok(WfExitValue::Normal("Invalid registry_id".to_string()));
    }

    let discovery_opts = ActivityOptions {
        activity_type: "run_registry_discovery_activity".to_string(),
        input: create_json_payload(&registry_id),
        start_to_close_timeout: Some(Duration::from_secs(300)),
        ..Default::default()
    };

    let discovery_res: DiscoveryResult = match execute_activity(&ctx, discovery_opts).await {
        Ok(res) => res,
        Err(e) => return Ok(WfExitValue::Normal(format!("Discovery Failed: {}", e))),
    };

    let touched_ids = discovery_res.touched_repo_ids;
    if touched_ids.is_empty() {
        return Ok(WfExitValue::Normal(format!(
            "Discovery Completed: new={}, updated={}, synced=0",
            discovery_res.new_count, discovery_res.updated_count
        )));
    }

    let mut synced = 0;
    for chunk in touched_ids.chunks(5) {
        let mut futures = Vec::new();
        for &repo_id in chunk {
            let sync_opts = ActivityOptions {
                activity_type: "sync_single_skill_activity".to_string(),
                input: create_json_payload(&repo_id),
                start_to_close_timeout: Some(Duration::from_secs(300)),
                ..Default::default()
            };
            futures.push(ctx.activity(sync_opts));
        }

        let results = join_all(futures).await;
        for res in results {
            if let Some(status) = res.status {
                match status {
                    Status::Completed(_) => synced += 1,
                    Status::Failed(f) => tracing::error!("Triggered sync failed: {:?}", f),
                    _ => tracing::error!("Triggered sync returned abnormal status"),
                }
            }
        }
    }

    Ok(WfExitValue::Normal(format!(
        "Trigger Completed: new={}, updated={}, synced={}",
        discovery_res.new_count, discovery_res.updated_count, synced
    )))
}
