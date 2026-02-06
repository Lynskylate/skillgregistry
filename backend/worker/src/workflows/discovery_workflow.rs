use crate::activities::discovery::DiscoveryResult;
use crate::workflows::{create_json_payload, execute_activity};
use std::time::Duration;
use temporalio_sdk::{ActivityOptions, WfContext, WfExitValue};

pub async fn discovery_workflow(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    // Accept optional workflow input for deterministic one-shot discovery in E2E.
    let queries = ctx
        .get_args()
        .first()
        .and_then(|payload| serde_json::from_slice::<Vec<String>>(&payload.data).ok())
        .filter(|q| !q.is_empty())
        .unwrap_or_else(|| vec!["topic:agent-skill".to_string()]);

    // 1. Discovery Activity
    let discovery_opts = ActivityOptions {
        activity_type: "discovery_activity".to_string(),
        input: create_json_payload(&queries),
        start_to_close_timeout: Some(Duration::from_secs(300)),
        ..Default::default()
    };

    let discovery_res: DiscoveryResult = match execute_activity(&ctx, discovery_opts).await {
        Ok(res) => res,
        Err(e) => return Ok(WfExitValue::Normal(format!("Discovery Failed: {}", e))),
    };

    Ok(WfExitValue::Normal(format!(
        "Discovery Completed: {:?}",
        discovery_res
    )))
}
