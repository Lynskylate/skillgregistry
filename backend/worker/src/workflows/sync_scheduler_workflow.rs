use crate::contracts;
use crate::workflows::{
    create_json_payload, execute_activity, execute_activity_batch, BatchActivityResult,
};
use std::time::Duration;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

pub async fn sync_scheduler_workflow(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let fetch_opts = ActivityOptions {
        activity_type: contracts::activities::FETCH_PENDING_SKILLS.to_string(),
        start_to_close_timeout: Some(Duration::from_secs(60)),
        input: create_json_payload(&()),
        ..Default::default()
    };

    let pending_ids: Vec<i32> = match execute_activity(&ctx, fetch_opts).await {
        Ok(res) => res,
        Err(e) => return Ok(WfExitValue::Normal(format!("Fetch Pending Failed: {}", e))),
    };

    tracing::info!(count = pending_ids.len(), "Found pending repos to sync");

    let outcomes = execute_activity_batch(
        &ctx,
        pending_ids.as_slice(),
        contracts::WORKFLOW_BATCH_CHUNK_SIZE,
        |id| ActivityOptions {
            activity_type: contracts::activities::SYNC_SINGLE_SKILL.to_string(),
            input: create_json_payload(&id),
            start_to_close_timeout: Some(Duration::from_secs(300)),
            ..Default::default()
        },
    )
    .await;

    let mut total_synced = 0;
    for outcome in outcomes {
        match outcome {
            BatchActivityResult::Completed { .. } => total_synced += 1,
            BatchActivityResult::Failed(err) => tracing::error!(error = %err, "Sync failed"),
            BatchActivityResult::Cancelled => tracing::error!("Sync was cancelled"),
            BatchActivityResult::Backoff => tracing::error!("Sync returned backoff status"),
            BatchActivityResult::MissingStatus => tracing::error!("Sync returned missing status"),
        }
    }

    Ok(WfExitValue::Normal(format!(
        "Scheduler Finished. Synced {} skills.",
        total_synced
    )))
}
