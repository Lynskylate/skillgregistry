use anyhow::Result;
use async_trait::async_trait;
use common::settings::Settings;
use std::str::FromStr;
use temporalio_client::{ClientOptions, WorkflowClientTrait, WorkflowOptions};
use temporalio_sdk_core::Url;

fn new_discovery_workflow_id() -> String {
    format!("discovery-{}", uuid::Uuid::new_v4())
}

fn new_sync_scheduler_workflow_id() -> String {
    format!("sync-sched-{}", uuid::Uuid::new_v4())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StartWorkflowRequest {
    task_queue: String,
    workflow_id: String,
    workflow_type: String,
}

fn build_client_options(server_url: &str) -> Result<ClientOptions> {
    Ok(ClientOptions::builder()
        .target_url(Url::from_str(server_url)?)
        .client_name("skill-starter")
        .client_version("0.1.0")
        .identity("skill-starter".to_string())
        .build())
}

fn default_start_options() -> WorkflowOptions {
    WorkflowOptions {
        id_reuse_policy:
            temporalio_common::protos::temporal::api::enums::v1::WorkflowIdReusePolicy::AllowDuplicate,
        ..Default::default()
    }
}

fn discovery_request(task_queue: &str, workflow_id: String) -> StartWorkflowRequest {
    StartWorkflowRequest {
        task_queue: task_queue.to_string(),
        workflow_id,
        workflow_type: "discovery_workflow".to_string(),
    }
}

fn sync_scheduler_request(task_queue: &str, workflow_id: String) -> StartWorkflowRequest {
    StartWorkflowRequest {
        task_queue: task_queue.to_string(),
        workflow_id,
        workflow_type: "sync_scheduler_workflow".to_string(),
    }
}

#[async_trait]
trait StartWorkflowClient {
    async fn start(&self, request: StartWorkflowRequest, options: WorkflowOptions) -> Result<()>;
}

#[async_trait]
impl<T> StartWorkflowClient for T
where
    T: WorkflowClientTrait + Send + Sync,
{
    async fn start(&self, request: StartWorkflowRequest, options: WorkflowOptions) -> Result<()> {
        self.start_workflow(
            vec![],
            request.task_queue,
            request.workflow_id,
            request.workflow_type,
            None,
            options,
        )
        .await?;
        Ok(())
    }
}

async fn start_default_workflows<C>(client: &C, task_queue: &str) -> Result<()>
where
    C: StartWorkflowClient + Sync,
{
    let discovery_id = new_discovery_workflow_id();
    let options = default_start_options();

    tracing::info!("Starting Discovery Workflow: {}", discovery_id);
    client
        .start(discovery_request(task_queue, discovery_id), options.clone())
        .await?;

    let sync_id = new_sync_scheduler_workflow_id();
    tracing::info!("Starting Sync Scheduler Workflow: {}", sync_id);
    client
        .start(sync_scheduler_request(task_queue, sync_id), options)
        .await?;

    tracing::info!("Workflows started successfully.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let settings = Settings::new()?;
    let server_url = settings.temporal.server_url;
    let task_queue = settings.temporal.task_queue;

    let client_options = build_client_options(&server_url)?;
    let client = client_options.connect("default", None).await?;

    start_default_workflows(&client, &task_queue).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::{mock, predicate, Sequence};

    mock! {
        StarterClient {}

        #[async_trait]
        impl StartWorkflowClient for StarterClient {
            async fn start(&self, request: StartWorkflowRequest, options: WorkflowOptions) -> Result<()>;
        }
    }

    #[test]
    fn generated_workflow_ids_use_expected_prefixes() {
        let discovery = new_discovery_workflow_id();
        let sync = new_sync_scheduler_workflow_id();

        assert!(discovery.starts_with("discovery-"));
        assert!(sync.starts_with("sync-sched-"));
        assert_ne!(discovery, sync);
    }

    #[test]
    fn helper_builders_return_expected_values() {
        let options = build_client_options("http://localhost:7233").unwrap();
        assert!(options.target_url.to_string().contains("localhost"));

        let err = build_client_options("not-a-url").unwrap_err().to_string();
        assert!(err.contains("relative URL") || err.contains("invalid"));

        let start_opts = default_start_options();
        assert_eq!(
            start_opts.id_reuse_policy,
            temporalio_common::protos::temporal::api::enums::v1::WorkflowIdReusePolicy::AllowDuplicate
        );

        let discovery = discovery_request("queue", "id-1".to_string());
        assert_eq!(
            discovery,
            StartWorkflowRequest {
                task_queue: "queue".to_string(),
                workflow_id: "id-1".to_string(),
                workflow_type: "discovery_workflow".to_string(),
            }
        );

        let sync = sync_scheduler_request("queue", "id-2".to_string());
        assert_eq!(
            sync,
            StartWorkflowRequest {
                task_queue: "queue".to_string(),
                workflow_id: "id-2".to_string(),
                workflow_type: "sync_scheduler_workflow".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn start_default_workflows_starts_discovery_then_sync() {
        let mut mock = MockStarterClient::new();
        let mut sequence = Sequence::new();

        mock.expect_start()
            .times(1)
            .withf(|request, options| {
                request.task_queue == "queue"
                    && request.workflow_type == "discovery_workflow"
                    && request.workflow_id.starts_with("discovery-")
                    && options.id_reuse_policy
                        == temporalio_common::protos::temporal::api::enums::v1::WorkflowIdReusePolicy::AllowDuplicate
            })
            .in_sequence(&mut sequence)
            .returning(|_, _| Ok(()));

        mock.expect_start()
            .times(1)
            .withf(|request, _| {
                request.task_queue == "queue"
                    && request.workflow_type == "sync_scheduler_workflow"
                    && request.workflow_id.starts_with("sync-sched-")
            })
            .in_sequence(&mut sequence)
            .returning(|_, _| Ok(()));

        start_default_workflows(&mock, "queue").await.unwrap();
    }

    #[tokio::test]
    async fn start_default_workflows_propagates_client_error() {
        let mut mock = MockStarterClient::new();

        mock.expect_start()
            .times(1)
            .with(predicate::always(), predicate::always())
            .returning(|_, _| Err(anyhow::anyhow!("start failed")));

        let err = start_default_workflows(&mock, "queue").await.unwrap_err();
        assert!(err.to_string().contains("start failed"));
    }

    #[test]
    fn main_returns_error_for_invalid_temporal_url() {
        std::env::set_var("SKILLREGISTRY_TEMPORAL__SERVER_URL", "not-a-url");
        let result = super::main();
        std::env::remove_var("SKILLREGISTRY_TEMPORAL__SERVER_URL");

        assert!(result.is_err());
    }
}
