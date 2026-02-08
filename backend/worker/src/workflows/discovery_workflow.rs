use crate::activities::discovery::DiscoveryResult;
use crate::contracts;
use crate::workflows::{
    create_json_payload, execute_activity, execute_activity_batch, BatchActivityResult,
};
use std::time::Duration;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

pub(crate) fn aggregate_discovery_outcomes(outcomes: Vec<BatchActivityResult>) -> (u32, u32) {
    let mut total_new = 0;
    let mut total_updated = 0;

    for outcome in outcomes {
        match outcome {
            BatchActivityResult::Completed {
                payload: Some(payload),
            } => {
                if let Ok(discovery) = serde_json::from_slice::<DiscoveryResult>(&payload) {
                    total_new += discovery.new_count;
                    total_updated += discovery.updated_count;
                }
            }
            BatchActivityResult::Completed { payload: None } => {
                tracing::error!("Discovery chunk completed without payload");
            }
            BatchActivityResult::Failed(err) => {
                tracing::error!(error = %err, "Discovery chunk failed");
            }
            BatchActivityResult::Cancelled => {
                tracing::error!("Discovery chunk was cancelled");
            }
            BatchActivityResult::Backoff => {
                tracing::error!("Discovery chunk returned backoff status");
            }
            BatchActivityResult::MissingStatus => {
                tracing::error!("Discovery chunk returned missing status");
            }
        }
    }

    (total_new, total_updated)
}

pub(crate) fn build_fetch_due_registry_options() -> ActivityOptions {
    ActivityOptions {
        activity_type: contracts::activities::FETCH_DUE_DISCOVERY_REGISTRIES.to_string(),
        input: create_json_payload(&()),
        start_to_close_timeout: Some(Duration::from_secs(60)),
        ..Default::default()
    }
}

pub(crate) fn build_registry_discovery_options(registry_id: i32) -> ActivityOptions {
    ActivityOptions {
        activity_type: contracts::activities::RUN_REGISTRY_DISCOVERY.to_string(),
        input: create_json_payload(&registry_id),
        start_to_close_timeout: Some(Duration::from_secs(300)),
        ..Default::default()
    }
}

pub(crate) fn format_discovery_summary(total_new: u32, total_updated: u32) -> String {
    format!(
        "Discovery Completed: new={}, updated={}",
        total_new, total_updated
    )
}

pub async fn discovery_workflow(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let fetch_opts = build_fetch_due_registry_options();

    let due_registry_ids: Vec<i32> = match execute_activity(&ctx, fetch_opts).await {
        Ok(ids) => ids,
        Err(e) => {
            return Ok(WfExitValue::Normal(format!(
                "Fetch Due Registries Failed: {}",
                e
            )))
        }
    };

    if due_registry_ids.is_empty() {
        return Ok(WfExitValue::Normal(
            "Discovery Completed: no due registries".to_string(),
        ));
    }

    let outcomes = execute_activity_batch(
        &ctx,
        due_registry_ids.as_slice(),
        contracts::WORKFLOW_BATCH_CHUNK_SIZE,
        build_registry_discovery_options,
    )
    .await;

    let (total_new, total_updated) = aggregate_discovery_outcomes(outcomes);

    Ok(WfExitValue::Normal(format_discovery_summary(
        total_new,
        total_updated,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_discovery_outcomes_counts_only_valid_payloads() {
        let valid_one = serde_json::to_vec(&DiscoveryResult {
            new_count: 2,
            updated_count: 1,
            touched_repo_ids: vec![1],
        })
        .unwrap();
        let valid_two = serde_json::to_vec(&DiscoveryResult {
            new_count: 0,
            updated_count: 3,
            touched_repo_ids: vec![2],
        })
        .unwrap();

        let outcomes = vec![
            BatchActivityResult::Completed {
                payload: Some(valid_one),
            },
            BatchActivityResult::Completed {
                payload: Some(b"not-json".to_vec()),
            },
            BatchActivityResult::Completed {
                payload: Some(valid_two),
            },
            BatchActivityResult::Completed { payload: None },
            BatchActivityResult::Failed("boom".to_string()),
            BatchActivityResult::Cancelled,
            BatchActivityResult::Backoff,
            BatchActivityResult::MissingStatus,
        ];

        let (new_count, updated_count) = aggregate_discovery_outcomes(outcomes);
        assert_eq!(new_count, 2);
        assert_eq!(updated_count, 4);
    }
    #[test]
    fn workflow_option_builders_use_expected_defaults() {
        let fetch_opts = build_fetch_due_registry_options();
        assert_eq!(
            fetch_opts.activity_type,
            contracts::activities::FETCH_DUE_DISCOVERY_REGISTRIES
        );
        assert_eq!(
            fetch_opts.start_to_close_timeout,
            Some(Duration::from_secs(60))
        );

        let run_opts = build_registry_discovery_options(42);
        assert_eq!(
            run_opts.activity_type,
            contracts::activities::RUN_REGISTRY_DISCOVERY
        );
        assert_eq!(
            run_opts.start_to_close_timeout,
            Some(Duration::from_secs(300))
        );
        let id: i32 = serde_json::from_slice(&run_opts.input.data).unwrap();
        assert_eq!(id, 42);

        assert_eq!(
            format_discovery_summary(3, 4),
            "Discovery Completed: new=3, updated=4"
        );
    }
}
