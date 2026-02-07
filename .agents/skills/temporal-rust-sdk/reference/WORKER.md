# Temporal Rust SDK: Worker Reference

This document describes how to configure and run a Worker to process Workflow and Activity tasks.

## 1. Runtime and Connection

A Worker requires a `CoreRuntime` and a connected `Client`.

```rust
use temporalio_sdk_core::{CoreRuntime, RuntimeOptions, Url, init_worker};
use temporalio_common::telemetry::TelemetryOptions;
use std::sync::Arc;

// 1. Initialize Runtime
let runtime = CoreRuntime::new_assume_tokio(
    RuntimeOptions::builder()
        .telemetry_options(TelemetryOptions::builder().build())
        .build()?
)?;

// 2. Connect Client (See CLIENT.md)
let client = server_options.connect("default", None).await?;
```

## 2. Worker Configuration

Configure the worker with the namespace, task queue, and versioning strategy.

```rust
use temporalio_common::worker::{WorkerConfig, WorkerTaskTypes, WorkerVersioningStrategy};

let worker_config = WorkerConfig::builder()
    .namespace("default")
    .task_queue("my-task-queue")
    .task_types(WorkerTaskTypes::all())
    .versioning_strategy(WorkerVersioningStrategy::None {
        build_id: "my-worker-build-v1".to_owned(),
    })
    .build()?;
```

## 3. Initialization and Registration

Initialize the core worker, wrap it in the high-level `Worker` struct, and register your types.

```rust
use temporalio_sdk::Worker;

// 1. Init Core Worker
let core_worker = init_worker(&runtime, worker_config, client)?;

// 2. Create High-Level Worker
let mut worker = Worker::new_from_core(Arc::new(core_worker), "my-task-queue");

// 3. Register Definitions
// Activities
worker.register_activity("say-hello-activity", say_hello);
worker.register_activity("process-order", process_order);

// Workflows
worker.register_wf("hello-world-workflow", hello_world_workflow);
worker.register_wf("order-workflow", order_workflow);
```

## 4. Running the Worker

The `run` method starts the worker loop and blocks until a shutdown signal is received.

```rust
info!("Worker started. Press Ctrl+C to stop.");
worker.run().await?;
```
