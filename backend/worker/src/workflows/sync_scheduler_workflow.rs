use crate::workflows::{create_json_payload, execute_activity};
use futures::future::join_all;
use std::time::Duration;
use temporalio_common::protos::coresdk::activity_result::activity_resolution::Status;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

pub async fn sync_scheduler_workflow(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    // 1. Fetch Pending IDs
    let fetch_opts = ActivityOptions {
        activity_type: "fetch_pending_skills_activity".to_string(),
        start_to_close_timeout: Some(Duration::from_secs(60)),
        input: create_json_payload(&()),
        ..Default::default()
    };

    let pending_ids: Vec<i32> = match execute_activity(&ctx, fetch_opts).await {
        Ok(res) => res,
        Err(e) => return Ok(WfExitValue::Normal(format!("Fetch Pending Failed: {}", e))),
    };

    tracing::info!("Found {} pending repos to sync", pending_ids.len());

    // 2. Schedule Syncs with Concurrency Limit
    // We use Concurrent Activities as a proxy for Child Workflows due to Prototype SDK limitations.

    let chunk_size = 5;
    let mut total_synced = 0;

    for chunk in pending_ids.chunks(chunk_size) {
        let mut futures = Vec::new();

        for &id in chunk {
            let sync_opts = ActivityOptions {
                activity_type: "sync_single_skill_activity".to_string(),
                input: create_json_payload(&id),
                start_to_close_timeout: Some(Duration::from_secs(300)),
                ..Default::default()
            };

            // Start activity (returns a Future)
            futures.push(ctx.activity(sync_opts));
        }

        let results = join_all(futures).await;

        for res in results {
            if let Some(status) = res.status {
                match status {
                    Status::Completed(_) => total_synced += 1,
                    Status::Failed(f) => tracing::error!("Sync failed: {:?}", f),
                    _ => tracing::error!("Sync abnormal status"),
                }
            }
        }
    }

    Ok(WfExitValue::Normal(format!(
        "Scheduler Finished. Synced {} skills.",
        total_synced
    )))
}
