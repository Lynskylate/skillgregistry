use crate::activities::discovery::DiscoveryResult;
use crate::contracts;
use crate::workflows::{
    create_json_payload, execute_activity, execute_activity_batch, BatchActivityResult,
};
use std::time::Duration;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

pub(crate) fn count_triggered_syncs(outcomes: Vec<BatchActivityResult>) -> i32 {
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
    synced
}

pub(crate) fn parse_registry_id_from_payloads(
    payloads: &[temporalio_common::protos::temporal::api::common::v1::Payload],
) -> Result<i32, &'static str> {
    if let Some(payload) = payloads.first() {
        let id = serde_json::from_slice::<i32>(&payload.data).unwrap_or(0);
        if id <= 0 {
            return Err("Invalid registry_id");
        }
        Ok(id)
    } else {
        Err("Missing registry_id input")
    }
}

pub(crate) fn build_trigger_discovery_options(registry_id: i32) -> ActivityOptions {
    ActivityOptions {
        activity_type: contracts::activities::RUN_REGISTRY_DISCOVERY.to_string(),
        input: create_json_payload(&registry_id),
        start_to_close_timeout: Some(Duration::from_secs(300)),
        ..Default::default()
    }
}

pub(crate) fn build_trigger_sync_options(repo_id: i32) -> ActivityOptions {
    ActivityOptions {
        activity_type: contracts::activities::SYNC_SINGLE_SKILL.to_string(),
        input: create_json_payload(&repo_id),
        start_to_close_timeout: Some(Duration::from_secs(300)),
        ..Default::default()
    }
}

pub(crate) fn format_trigger_summary(new_count: u32, updated_count: u32, synced: i32) -> String {
    format!(
        "Trigger Completed: new={}, updated={}, synced={}",
        new_count, updated_count, synced
    )
}

pub async fn trigger_registry_workflow(
    ctx: WfContext,
) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let registry_id = match parse_registry_id_from_payloads(args) {
        Ok(id) => id,
        Err(msg) => return Ok(WfExitValue::Normal(msg.to_string())),
    };

    let discovery_opts = build_trigger_discovery_options(registry_id);

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
        build_trigger_sync_options,
    )
    .await;

    let synced = count_triggered_syncs(outcomes);

    Ok(WfExitValue::Normal(format_trigger_summary(
        discovery_res.new_count,
        discovery_res.updated_count,
        synced,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflows::create_json_payload;

    #[test]
    fn parse_registry_id_from_payloads_handles_missing_and_invalid_data() {
        let missing = parse_registry_id_from_payloads(&[]);
        assert_eq!(missing.unwrap_err(), "Missing registry_id input");

        let invalid_payload = temporalio_common::protos::temporal::api::common::v1::Payload {
            data: b"not-json".to_vec(),
            ..Default::default()
        };
        let invalid = parse_registry_id_from_payloads(&[invalid_payload]);
        assert_eq!(invalid.unwrap_err(), "Invalid registry_id");

        let zero = parse_registry_id_from_payloads(&[create_json_payload(&0)]);
        assert_eq!(zero.unwrap_err(), "Invalid registry_id");

        let ok = parse_registry_id_from_payloads(&[create_json_payload(&42)]).unwrap();
        assert_eq!(ok, 42);
    }

    #[test]
    fn count_triggered_syncs_counts_completed_only() {
        let outcomes = vec![
            BatchActivityResult::Completed { payload: None },
            BatchActivityResult::Completed {
                payload: Some(vec![1]),
            },
            BatchActivityResult::Failed("boom".to_string()),
            BatchActivityResult::Cancelled,
            BatchActivityResult::Backoff,
            BatchActivityResult::MissingStatus,
        ];

        assert_eq!(count_triggered_syncs(outcomes), 2);
    }
    #[test]
    fn trigger_option_helpers_encode_ids_and_summary() {
        let discovery = build_trigger_discovery_options(7);
        assert_eq!(
            discovery.activity_type,
            contracts::activities::RUN_REGISTRY_DISCOVERY
        );
        assert_eq!(
            discovery.start_to_close_timeout,
            Some(Duration::from_secs(300))
        );
        let id: i32 = serde_json::from_slice(&discovery.input.data).unwrap();
        assert_eq!(id, 7);

        let sync = build_trigger_sync_options(8);
        assert_eq!(sync.activity_type, contracts::activities::SYNC_SINGLE_SKILL);
        let repo_id: i32 = serde_json::from_slice(&sync.input.data).unwrap();
        assert_eq!(repo_id, 8);

        assert_eq!(
            format_trigger_summary(1, 2, 3),
            "Trigger Completed: new=1, updated=2, synced=3"
        );
    }
}
