# Temporal Rust SDK: Activity Reference

This document describes how to write Activities.

## 1. Definition

An Activity is an `async` function that takes an `ActContext` and an input argument, returning a `Result`.

```rust
use temporalio_sdk::{ActContext, ActivityError};
use anyhow::Result;

async fn my_activity(_ctx: ActContext, input: String) -> Result<String, ActivityError> {
    // Activity logic
    println!("Processing: {}", input);
    Ok(format!("Processed {}", input))
}
```

## 2. Error Handling

Return `ActivityError` to indicate failure.

- `ActivityError::from(anyhow::Error)`: Generic failure (retryable by default).
- `ActivityError::NonRetryable(anyhow::Error)`: Fatal error.

```rust
use anyhow::anyhow;

async fn failure_activity(_ctx: ActContext, _: ()) -> Result<(), ActivityError> {
    // Retryable
    Err(anyhow!("Network glitch").into())
    
    // Non-Retryable
    // Err(ActivityError::NonRetryable(anyhow!("Invalid config")))
}
```

## 3. Local Activities

Local Activities execute on the same Worker as the Workflow and avoid scheduling/polling overhead with the Temporal service.

Use Local Activities for fast operations that do not require external calls (or can be safely retried and are effectively idempotent).

```rust
use std::time::Duration;
use temporalio_sdk::{LocalActivityOptions, WfContext};

async fn example_workflow(ctx: WfContext) -> temporalio_sdk::WorkflowResult<String> {
    let payload = temporalio_common::protos::temporal::api::common::v1::Payload {
        metadata: std::collections::HashMap::from([(
            "encoding".to_string(),
            "json/plain".as_bytes().to_vec(),
        )]),
        data: serde_json::to_vec(&"input").unwrap(),
        ..Default::default()
    };

    let opts = LocalActivityOptions {
        activity_type: "sanitize-name".to_string(),
        start_to_close_timeout: Some(Duration::from_secs(2)),
        input: payload,
        ..Default::default()
    };

    let _res = ctx.local_activity(opts).await;
    Ok(temporalio_sdk::WfExitValue::Normal("ok".to_string()))
}
```
