use crate::activities::discovery::DiscoveryResult;
use crate::contracts;
use crate::workflows::{
    create_json_payload, execute_activity, execute_activity_batch, BatchActivityResult,
};
use std::time::Duration;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

pub async fn discovery_workflow(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let fetch_opts = ActivityOptions {
        activity_type: contracts::activities::FETCH_DUE_DISCOVERY_REGISTRIES.to_string(),
        input: create_json_payload(&()),
        start_to_close_timeout: Some(Duration::from_secs(60)),
        ..Default::default()
    };

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

    let mut total_new = 0;
    let mut total_updated = 0;

    let outcomes = execute_activity_batch(
        &ctx,
        due_registry_ids.as_slice(),
        contracts::WORKFLOW_BATCH_CHUNK_SIZE,
        |registry_id| ActivityOptions {
            activity_type: contracts::activities::RUN_REGISTRY_DISCOVERY.to_string(),
            input: create_json_payload(&registry_id),
            start_to_close_timeout: Some(Duration::from_secs(300)),
            ..Default::default()
        },
    )
    .await;

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

    Ok(WfExitValue::Normal(format!(
        "Discovery Completed: new={}, updated={}",
        total_new, total_updated
    )))
}
