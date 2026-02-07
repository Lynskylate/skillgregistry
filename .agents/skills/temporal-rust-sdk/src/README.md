# Temporal SDK Helpers

This directory contains helper utilities for the Temporal Rust SDK, primarily providing a simplified Activity registration API.

## Directory Structure

```
.agent/skills/temporal-rust-sdk/
├── SKILL.md                          # Primary documentation (updated)
├── src/
│   ├── lib.rs                        # Simplified API implementation
│   └── Cargo.toml                    # Helper library configuration
├── reference/
│   ├── ACTIVITY_REGISTRATION.md      # Detailed Activity registration guide
│   ├── CLIENT.md                     # Client reference
│   ├── WORKER.md                     # Worker reference
│   ├── WORKFLOW.md                   # Workflow reference
│   └── ACTIVITY.md                   # Activity reference
└── examples/                         # Example code
    ├── struct-activity/              # Example: Activity as a struct method
    └── ...
```

## Quick Start

### 1. Copy helper code

Copy the contents of `src/lib.rs` into your project:

```bash
cp .agent/skills/temporal-rust-sdk/src/lib.rs your-project/src/temporal_helpers.rs
```

### 2. Use the simplified API

```rust
// In your project
mod temporal_helpers;
use temporal_helpers::ActivityRegistrarJson;

#[derive(Clone)]
struct MyService {
    // Dependencies
}

impl MyService {
    async fn process(&self, input: MyInput) -> Result<MyOutput, ActivityError> {
        // Business logic
    }
}

// Register Activity (only 3 lines!)
let service = Arc::new(MyService::new());
let svc = Arc::clone(&service);
worker.register_activity_json("my-activity", move |_ctx, input: MyInput| {
    let svc = Arc::clone(&svc);
    async move { svc.process(input).await }
});
```

## Core Features

### ActivityRegistrarJson Trait

Trait for Activity registration with automatic JSON serialization.

```rust
pub trait ActivityRegistrarJson {
    fn register_activity_json<Input, Output, Fut>(
        &mut self,
        activity_type: impl Into<String>,
        f: impl Fn(ActContext, Input) -> Fut + Send + Sync + Clone + 'static,
    )
    where
        Input: DeserializeOwned + Send + 'static,
        Output: Serialize + Send + 'static,
        Fut: Future<Output = Result<Output, ActivityError>> + Send + 'static;
}
```

### register_activity! macro

An even more concise registration approach:

```rust
register_activity!(worker, "activity-name", service, method_name);
```

## Comparison

| Approach | Lines of Code | Description |
|------|-------|------|
| Raw API | ~25 lines | Manual serialization |
| Simplified API | 3 lines | Automatic JSON serialization |
| Macro-Based | 1 line | Most concise |

## Documentation

- [**Activity Registration Guide**](reference/ACTIVITY_REGISTRATION.md) - Detailed Activity registration guide
- [**SKILL.md**](SKILL.md) - Primary skill document

## Examples

### Basic example

```rust
use temporal_sdk_helpers::ActivityRegistrarJson;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Define input/output
#[derive(Serialize, Deserialize)]
struct GreetInput {
    name: String,
}

#[derive(Serialize, Deserialize)]
struct GreetOutput {
    message: String,
}

// Define service
#[derive(Clone)]
struct GreeterService {
    prefix: String,
}

impl GreeterService {
    async fn greet(&self, input: GreetInput) -> Result<GreetOutput, ActivityError> {
        Ok(GreetOutput {
            message: format!("{} {}!", self.prefix, input.name),
        })
    }
}

// Register Activity
let service = Arc::new(GreeterService { prefix: "Hello".to_string() });
let svc = Arc::clone(&service);
worker.register_activity_json("greet", move |_ctx, input: GreetInput| {
    let svc = Arc::clone(&svc);
    async move { svc.greet(input).await }
});
```

### Multiple Activities

```rust
// Register multiple Activities — one per line, very concise
let svc = Arc::clone(&service);
worker.register_activity_json("create-order", move |ctx, input: CreateOrderInput| {
    let svc = Arc::clone(&svc);
    async move { svc.create_order(input).await }
});

let svc = Arc::clone(&service);
worker.register_activity_json("cancel-order", move |ctx, input: CancelOrderInput| {
    let svc = Arc::clone(&svc);
    async move { svc.cancel_order(input).await }
});

let svc = Arc::clone(&service);
worker.register_activity_json("process-payment", move |ctx, input: PaymentInput| {
    let svc = Arc::clone(&svc);
    async move { svc.process_payment(input).await }
});
```

## Advantages

1. Reduce boilerplate by 88%: from ~25 lines down to 3
2. Compile-time type safety: serde handled automatically, type errors caught at compile time
3. Automatic JSON serialization: no manual Payload handling required
4. Dependency injection support: share service dependencies via Arc
5. Backward compatible: raw API remains available

## License

MIT
