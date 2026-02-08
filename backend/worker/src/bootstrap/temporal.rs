use anyhow::Result;
use common::settings::Settings;
use std::str::FromStr;
use temporalio_client::ClientOptions;
use temporalio_common::worker::{WorkerConfig, WorkerTaskTypes, WorkerVersioningStrategy};
use temporalio_sdk::Worker;
use temporalio_sdk_core::{init_worker, CoreRuntime, RuntimeOptions, Url};

pub struct TemporalWorkerRuntime {
    pub worker: Worker,
    // Keep runtime alive for the lifetime of the worker.
    _runtime: CoreRuntime,
}

fn get_host_name() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "Unknown".to_string())
}

fn get_worker_identity(task_queue: &str) -> String {
    format!("{}@{}@{}", std::process::id(), get_host_name(), task_queue)
}

pub async fn build_temporal_worker(settings: &Settings) -> Result<TemporalWorkerRuntime> {
    let server_url = settings.temporal.server_url.clone();
    let task_queue = settings.temporal.task_queue.as_str();
    let worker_identity = get_worker_identity(task_queue);

    let server_options = ClientOptions::builder()
        .target_url(Url::from_str(&server_url)?)
        .client_name("skill-worker")
        .client_version("0.1.0")
        .identity(worker_identity)
        .build();

    let client = server_options.connect("default", None).await?;

    let runtime_options = RuntimeOptions::builder()
        .build()
        .map_err(|e| anyhow::anyhow!(e))?;
    let runtime = CoreRuntime::new_assume_tokio(runtime_options).map_err(|e| anyhow::anyhow!(e))?;

    let worker_config = WorkerConfig::builder()
        .namespace("default")
        .task_queue(task_queue)
        .task_types(WorkerTaskTypes::all())
        .versioning_strategy(WorkerVersioningStrategy::None {
            build_id: "rust-worker-0.1.0".to_string(),
        })
        .build()
        .map_err(|e| anyhow::anyhow!(e))?;

    let core_worker = init_worker(&runtime, worker_config, client)?;
    let worker = Worker::new_from_core(std::sync::Arc::new(core_worker), task_queue);

    Ok(TemporalWorkerRuntime {
        worker,
        _runtime: runtime,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_host_name_returns_non_empty_value() {
        let host = get_host_name();
        assert!(!host.trim().is_empty());
    }

    #[test]
    fn get_worker_identity_contains_pid_host_and_task_queue() {
        let identity = get_worker_identity("queue-a");
        let parts: Vec<&str> = identity.split('@').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts[0].parse::<u32>().is_ok());
        assert!(!parts[1].is_empty());
        assert_eq!(parts[2], "queue-a");
    }
}
