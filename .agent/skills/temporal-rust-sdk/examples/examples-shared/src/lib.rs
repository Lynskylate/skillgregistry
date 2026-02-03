use anyhow::{anyhow, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::str::FromStr;
use std::time::Duration;
use temporalio_client::{ClientOptions, WorkflowClientTrait};
use temporalio_common::protos::temporal::api::common::v1::Payload;
use temporalio_common::protos::temporal::api::enums::v1::EventType;
use temporalio_common::protos::temporal::api::history::v1::history_event;
use temporalio_common::telemetry::TelemetryOptions;
use temporalio_sdk_core::{CoreRuntime, RuntimeOptions, Url};

pub fn temporal_server_url() -> Result<Url> {
    let raw = std::env::var("TEMPORAL_SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:7233".to_string());
    Ok(Url::from_str(&raw)?)
}

pub fn temporal_namespace() -> String {
    std::env::var("TEMPORAL_NAMESPACE").unwrap_or_else(|_| "default".to_string())
}

pub fn build_client_options(
    client_name: &str,
    client_version: &str,
    identity: &str,
) -> Result<ClientOptions> {
    Ok(ClientOptions::builder()
        .target_url(temporal_server_url()?)
        .client_name(client_name)
        .client_version(client_version)
        .identity(identity.to_string())
        .build())
}

pub fn init_runtime() -> Result<CoreRuntime> {
    let telemetry_options = TelemetryOptions::builder().build();
    let runtime_options = RuntimeOptions::builder()
        .telemetry_options(telemetry_options)
        .build()
        .map_err(|e| anyhow!(e))?;
    CoreRuntime::new_assume_tokio(runtime_options).map_err(|e| anyhow!(e))
}

pub fn json_payload<T: Serialize>(value: &T) -> Result<Payload> {
    Ok(Payload {
        metadata: std::collections::HashMap::from([(
            "encoding".to_string(),
            "json/plain".as_bytes().to_vec(),
        )]),
        data: serde_json::to_vec(value)?,
        ..Default::default()
    })
}

pub fn from_json_payload<T: DeserializeOwned>(payload: &Payload) -> Result<T> {
    Ok(serde_json::from_slice(&payload.data)?)
}

pub async fn poll_workflow_result<T, C>(
    client: &C,
    workflow_id: &str,
    run_id: &str,
    poll_interval: Duration,
) -> Result<Option<T>>
where
    T: DeserializeOwned,
    C: WorkflowClientTrait + Sync,
{
    loop {
        tokio::time::sleep(poll_interval).await;

        let history_response = client
            .get_workflow_execution_history(
                workflow_id.to_string(),
                Some(run_id.to_string()),
                vec![],
            )
            .await?;

        let Some(history) = history_response.history else {
            continue;
        };

        for event in history.events {
            if event.event_type == EventType::WorkflowExecutionCompleted as i32 {
                if let Some(history_event::Attributes::WorkflowExecutionCompletedEventAttributes(
                    attrs,
                )) = event.attributes
                {
                    let Some(payloads) = attrs.result else {
                        return Ok(None);
                    };
                    let Some(payload) = payloads.payloads.first() else {
                        return Ok(None);
                    };

                    let result = serde_json::from_slice::<T>(&payload.data)?;
                    return Ok(Some(result));
                }

                return Ok(None);
            }

            if event.event_type == EventType::WorkflowExecutionFailed as i32 {
                return Err(anyhow!("Workflow failed"));
            }
        }
    }
}
