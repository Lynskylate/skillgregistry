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
