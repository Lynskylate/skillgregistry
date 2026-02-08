use crate::activities::discovery::DiscoveryResult;
use crate::contracts;
use crate::workflows::{
    create_json_payload, execute_activity, execute_activity_batch, BatchActivityResult,
};
use std::time::Duration;
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
        activity_type: contracts::activities::RUN_REGISTRY_DISCOVERY.to_string(),
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

    let outcomes = execute_activity_batch(
        &ctx,
        touched_ids.as_slice(),
        contracts::WORKFLOW_BATCH_CHUNK_SIZE,
        |repo_id| ActivityOptions {
            activity_type: contracts::activities::SYNC_SINGLE_SKILL.to_string(),
            input: create_json_payload(&repo_id),
            start_to_close_timeout: Some(Duration::from_secs(300)),
            ..Default::default()
        },
    )
    .await;

    let mut synced = 0;
    for outcome in outcomes {
        match outcome {
            BatchActivityResult::Completed { .. } => synced += 1,
            BatchActivityResult::Failed(err) => {
                tracing::error!(error = %err, "Triggered sync failed")
            }
            BatchActivityResult::Cancelled => tracing::error!("Triggered sync was cancelled"),
            BatchActivityResult::Backoff => tracing::error!("Triggered sync returned backoff"),
            BatchActivityResult::MissingStatus => {
                tracing::error!("Triggered sync returned missing status")
            }
        }
    }

    Ok(WfExitValue::Normal(format!(
        "Trigger Completed: new={}, updated={}, synced={}",
        discovery_res.new_count, discovery_res.updated_count, synced
    )))
}
