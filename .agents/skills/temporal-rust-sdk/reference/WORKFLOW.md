# Temporal Rust SDK: Workflow Reference

This document describes how to write Workflows using the `temporalio_sdk` prototype.

## 1. Definition

A Workflow is an `async` function that takes a `WfContext` and returns a `WorkflowResult`.

```rust
use temporalio_sdk::{WfContext, WfExitValue, WorkflowResult};

async fn my_workflow(ctx: WfContext) -> WorkflowResult<String> {
    // Workflow logic here
    Ok(WfExitValue::Normal("Result".to_string()))
}
```

## 2. Executing Activities

Use `ctx.activity` with `ActivityOptions`. Note that you must construct the input Payload manually in this version.

```rust
use temporalio_sdk::ActivityOptions;
use std::time::Duration;

let payload = temporalio_common::protos::temporal::api::common::v1::Payload {
    metadata: std::collections::HashMap::from([("encoding".to_string(), "json/plain".as_bytes().to_vec())]),
    data: serde_json::to_vec(&"input-arg").unwrap(),
    ..Default::default()
};

let opts = ActivityOptions {
    activity_type: "my-activity".to_string(),
    start_to_close_timeout: Some(Duration::from_secs(5)),
    task_queue: None,
    input: payload,
    ..Default::default()
};

let res = ctx.activity(opts).await;

if let Some(status) = res.status {
    // Check for Completed, Failed, etc.
}
```

## 3. Timers

Use `ctx.timer(duration)` to sleep or set timeouts.

```rust
// Sleep for 5 seconds
ctx.timer(Duration::from_secs(5)).await;
```

## 4. Signal Handling

Use `ctx.make_signal_channel` to listen for signals.

```rust
let mut signal_chan = ctx.make_signal_channel("my-signal");

if let Some(signal) = signal_chan.next().await {
    // Process signal
}
```

## 5. Select Loop (Event Loop)

Use `tokio::select!` to handle multiple asynchronous events (like Signals vs Timers) deterministically.

```rust
loop {
    let mut timer = ctx.timer(Duration::from_secs(10));
    let mut signal_recv = signal_chan.next();

    tokio::select! {
        Some(signal) = signal_recv => {
            timer.cancel(&ctx); // Cancel timer if signal arrives first
            // Handle signal
        }
        _ = &mut timer => {
            // Handle timeout
        }
    }
}
```
