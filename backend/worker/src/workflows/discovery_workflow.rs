use crate::activities::discovery::DiscoveryResult;
use crate::workflows::{create_json_payload, execute_activity};
use futures::future::join_all;
use std::time::Duration;
use temporalio_common::protos::coresdk::activity_result::activity_resolution::Status;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

pub async fn discovery_workflow(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let fetch_opts = ActivityOptions {
        activity_type: "fetch_due_discovery_registries_activity".to_string(),
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
    let chunk_size = 5;

    for chunk in due_registry_ids.chunks(chunk_size) {
        let mut futures = Vec::new();
        for &registry_id in chunk {
            let opts = ActivityOptions {
                activity_type: "run_registry_discovery_activity".to_string(),
                input: create_json_payload(&registry_id),
                start_to_close_timeout: Some(Duration::from_secs(300)),
                ..Default::default()
            };
            futures.push(ctx.activity(opts));
        }

        for res in join_all(futures).await {
            if let Some(status) = res.status {
                match status {
                    Status::Completed(success) => {
                        if let Some(payload) = success.result {
                            if let Ok(discovery) =
                                serde_json::from_slice::<DiscoveryResult>(&payload.data)
                            {
                                total_new += discovery.new_count;
                                total_updated += discovery.updated_count;
                            }
                        }
                    }
                    Status::Failed(f) => tracing::error!("Discovery chunk failed: {:?}", f),
                    _ => tracing::error!("Discovery chunk returned abnormal status"),
                }
            }
        }
    }

    Ok(WfExitValue::Normal(format!(
        "Discovery Completed: new={}, updated={}",
        total_new, total_updated
    )))
}
