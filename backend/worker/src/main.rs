#![allow(dead_code)]
mod activities;
mod bootstrap;
mod contracts;
mod github;
#[cfg(test)]
mod index_flow_tests;
mod ports;
mod sync;
mod workflows;

use bootstrap::{
    build_temporal_worker, build_worker_context, build_worker_services, register_activities,
    register_workflows,
};
use common::settings::Settings;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "worker=debug,common=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let settings = Settings::new()?;

    let s3_bucket = settings.s3.bucket.clone();
    let s3_region = settings.s3.region.clone();
    let s3_endpoint = settings.s3.endpoint.clone();
    tracing::info!(
        s3_bucket = %s3_bucket,
        s3_region = %s3_region,
        s3_endpoint = %s3_endpoint.clone().unwrap_or_else(|| "<aws-default>".to_string()),
        "S3 config loaded"
    );

    let ctx = build_worker_context(settings).await?;
    let worker_services = build_worker_services(&ctx);

    let task_queue = ctx.settings.temporal.task_queue.clone();
    let mut temporal_runtime = build_temporal_worker(ctx.settings.as_ref()).await?;

    register_activities(
        &mut temporal_runtime.worker,
        worker_services.discovery,
        worker_services.sync,
    );
    register_workflows(&mut temporal_runtime.worker);

    tracing::info!(%task_queue, "Starting Temporal Worker");
    temporal_runtime.worker.run().await?;

    Ok(())
}
