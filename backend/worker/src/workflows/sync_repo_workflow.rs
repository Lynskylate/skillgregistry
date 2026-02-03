use crate::activities::sync::SyncResult;
use crate::workflows::{create_json_payload, execute_activity};
use std::time::Duration;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

pub async fn sync_repo_workflow(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    // Get input (registry_id)
    let args = ctx.get_args();
    let registry_id: i32 = if let Some(payload) = args.first() {
        serde_json::from_slice(&payload.data).unwrap_or(0)
    } else {
        return Ok(WfExitValue::Normal("Missing registry_id input".to_string()));
    };

    if registry_id == 0 {
        return Ok(WfExitValue::Normal("Invalid registry_id".to_string()));
    }

    let sync_opts = ActivityOptions {
        activity_type: "sync_single_skill_activity".to_string(),
        input: create_json_payload(&registry_id),
        start_to_close_timeout: Some(Duration::from_secs(300)),
        ..Default::default()
    };

    let res: SyncResult = match execute_activity(&ctx, sync_opts).await {
        Ok(res) => res,
        Err(e) => return Ok(WfExitValue::Normal(format!("Sync Failed: {}", e))),
    };

    Ok(WfExitValue::Normal(format!("Sync Completed: {:?}", res)))
}
