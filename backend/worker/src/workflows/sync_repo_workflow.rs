use crate::activities::sync::{SnapshotResult, SyncResult};
use crate::contracts;
use crate::workflows::{create_json_payload, execute_local_activity};
use std::time::Duration;
use temporalio_common::protos::temporal::api::common::v1::RetryPolicy;
use temporalio_sdk::{LocalActivityOptions, WfContext, WfExitValue};

pub(crate) fn parse_registry_id_from_payloads(
    payloads: &[temporalio_common::protos::temporal::api::common::v1::Payload],
) -> Result<i32, &'static str> {
    if let Some(payload) = payloads.first() {
        let id = serde_json::from_slice::<i32>(&payload.data).unwrap_or(0);
        if id == 0 {
            return Err("Invalid registry_id");
        }
        Ok(id)
    } else {
        Err("Missing registry_id input")
    }
}

pub(crate) fn sync_retry_policy() -> RetryPolicy {
    RetryPolicy {
        maximum_attempts: 5,
        ..Default::default()
    }
}

pub(crate) fn build_fetch_snapshot_options(
    registry_id: i32,
    retry_policy: RetryPolicy,
) -> LocalActivityOptions {
    LocalActivityOptions {
        activity_type: contracts::activities::FETCH_REPO_SNAPSHOT.to_string(),
        input: create_json_payload(&registry_id),
        start_to_close_timeout: Some(Duration::from_secs(120)),
        schedule_to_close_timeout: Some(Duration::from_secs(180)),
        retry_policy,
        timer_backoff_threshold: Some(Duration::from_secs(5)),
        ..Default::default()
    }
}

pub(crate) fn build_apply_snapshot_options(
    snapshot: &crate::activities::sync::RepoSnapshotRef,
    retry_policy: RetryPolicy,
) -> LocalActivityOptions {
    LocalActivityOptions {
        activity_type: contracts::activities::APPLY_SYNC_FROM_SNAPSHOT.to_string(),
        input: create_json_payload(snapshot),
        start_to_close_timeout: Some(Duration::from_secs(600)),
        schedule_to_close_timeout: Some(Duration::from_secs(900)),
        retry_policy,
        timer_backoff_threshold: Some(Duration::from_secs(5)),
        ..Default::default()
    }
}

pub(crate) fn format_sync_completed(res: &SyncResult) -> String {
    format!("Sync Completed: {:?}", res)
}

pub async fn sync_repo_workflow(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    // Get input (registry_id)
    let args = ctx.get_args();
    let registry_id = match parse_registry_id_from_payloads(args) {
        Ok(id) => id,
        Err(msg) => return Ok(WfExitValue::Normal(msg.to_string())),
    };

    let retry_policy = sync_retry_policy();

    let snapshot_opts = build_fetch_snapshot_options(registry_id, retry_policy.clone());

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

    let apply_opts = build_apply_snapshot_options(&snapshot, retry_policy);

    let res: SyncResult = match execute_local_activity(&ctx, apply_opts).await {
        Ok(res) => res,
        Err(e) => return Ok(WfExitValue::Normal(format!("Sync Failed: {}", e))),
    };

    Ok(WfExitValue::Normal(format_sync_completed(&res)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflows::create_json_payload;

    #[test]
    fn parse_registry_id_from_payloads_handles_valid_and_invalid_values() {
        let missing = parse_registry_id_from_payloads(&[]);
        assert_eq!(missing.unwrap_err(), "Missing registry_id input");

        let invalid = parse_registry_id_from_payloads(&[
            temporalio_common::protos::temporal::api::common::v1::Payload {
                data: b"nope".to_vec(),
                ..Default::default()
            },
        ]);
        assert_eq!(invalid.unwrap_err(), "Invalid registry_id");

        let zero = parse_registry_id_from_payloads(&[create_json_payload(&0)]);
        assert_eq!(zero.unwrap_err(), "Invalid registry_id");

        let ok = parse_registry_id_from_payloads(&[create_json_payload(&7)]).unwrap();
        assert_eq!(ok, 7);
    }
    #[test]
    fn option_builders_encode_payloads_and_timeouts() {
        let retry = sync_retry_policy();
        assert_eq!(retry.maximum_attempts, 5);

        let fetch = build_fetch_snapshot_options(11, retry.clone());
        assert_eq!(
            fetch.activity_type,
            contracts::activities::FETCH_REPO_SNAPSHOT
        );
        assert_eq!(fetch.start_to_close_timeout, Some(Duration::from_secs(120)));
        assert_eq!(
            fetch.schedule_to_close_timeout,
            Some(Duration::from_secs(180))
        );
        let id: i32 = serde_json::from_slice(&fetch.input.data).unwrap();
        assert_eq!(id, 11);

        let snapshot = crate::activities::sync::RepoSnapshotRef {
            registry_id: 11,
            owner: "acme".to_string(),
            name: "skills".to_string(),
            url: "https://github.com/acme/skills".to_string(),
            zip_hash: "hash".to_string(),
            snapshot_s3_key: "repo-snapshots/hash.zip".to_string(),
        };
        let apply = build_apply_snapshot_options(&snapshot, retry);
        assert_eq!(
            apply.activity_type,
            contracts::activities::APPLY_SYNC_FROM_SNAPSHOT
        );
        assert_eq!(apply.start_to_close_timeout, Some(Duration::from_secs(600)));
        assert_eq!(
            apply.schedule_to_close_timeout,
            Some(Duration::from_secs(900))
        );

        let decoded: crate::activities::sync::RepoSnapshotRef =
            serde_json::from_slice(&apply.input.data).unwrap();
        assert_eq!(decoded.registry_id, 11);

        let summary = format_sync_completed(&SyncResult {
            status: "Updated".to_string(),
            version: Some("1.0.0".to_string()),
        });
        assert!(summary.contains("Sync Completed"));
        assert!(summary.contains("Updated"));
    }
}
