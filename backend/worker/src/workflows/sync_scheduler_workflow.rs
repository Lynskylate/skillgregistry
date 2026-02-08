use crate::contracts;
use crate::workflows::{
    create_json_payload, execute_activity, execute_activity_batch, BatchActivityResult,
};
use std::time::Duration;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

pub(crate) fn count_synced_from_outcomes(outcomes: Vec<BatchActivityResult>) -> i32 {
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
    total_synced
}

pub(crate) fn build_fetch_pending_options() -> ActivityOptions {
    ActivityOptions {
        activity_type: contracts::activities::FETCH_PENDING_SKILLS.to_string(),
        start_to_close_timeout: Some(Duration::from_secs(60)),
        input: create_json_payload(&()),
        ..Default::default()
    }
}

pub(crate) fn build_sync_single_options(id: i32) -> ActivityOptions {
    ActivityOptions {
        activity_type: contracts::activities::SYNC_SINGLE_SKILL.to_string(),
        input: create_json_payload(&id),
        start_to_close_timeout: Some(Duration::from_secs(300)),
        ..Default::default()
    }
}

pub(crate) fn format_sync_scheduler_summary(total_synced: i32) -> String {
    format!("Scheduler Finished. Synced {} skills.", total_synced)
}

pub async fn sync_scheduler_workflow(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let fetch_opts = build_fetch_pending_options();

    let pending_ids: Vec<i32> = match execute_activity(&ctx, fetch_opts).await {
        Ok(res) => res,
        Err(e) => return Ok(WfExitValue::Normal(format!("Fetch Pending Failed: {}", e))),
    };

    tracing::info!(count = pending_ids.len(), "Found pending repos to sync");

    let outcomes = execute_activity_batch(
        &ctx,
        pending_ids.as_slice(),
        contracts::WORKFLOW_BATCH_CHUNK_SIZE,
        build_sync_single_options,
    )
    .await;

    let total_synced = count_synced_from_outcomes(outcomes);

    Ok(WfExitValue::Normal(format_sync_scheduler_summary(
        total_synced,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_synced_from_outcomes_only_counts_completed() {
        let outcomes = vec![
            BatchActivityResult::Completed { payload: None },
            BatchActivityResult::Completed {
                payload: Some(vec![1, 2, 3]),
            },
            BatchActivityResult::Failed("boom".to_string()),
            BatchActivityResult::Cancelled,
            BatchActivityResult::Backoff,
            BatchActivityResult::MissingStatus,
        ];

        assert_eq!(count_synced_from_outcomes(outcomes), 2);
    }
    #[test]
    fn scheduler_option_helpers_build_expected_payloads() {
        let fetch = build_fetch_pending_options();
        assert_eq!(
            fetch.activity_type,
            contracts::activities::FETCH_PENDING_SKILLS
        );
        assert_eq!(fetch.start_to_close_timeout, Some(Duration::from_secs(60)));

        let sync_one = build_sync_single_options(99);
        assert_eq!(
            sync_one.activity_type,
            contracts::activities::SYNC_SINGLE_SKILL
        );
        assert_eq!(
            sync_one.start_to_close_timeout,
            Some(Duration::from_secs(300))
        );
        let id: i32 = serde_json::from_slice(&sync_one.input.data).unwrap();
        assert_eq!(id, 99);

        assert_eq!(
            format_sync_scheduler_summary(5),
            "Scheduler Finished. Synced 5 skills."
        );
    }
}
