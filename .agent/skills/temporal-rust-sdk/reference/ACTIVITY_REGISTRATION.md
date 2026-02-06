# Activity Registration Guide

This guide describes multiple ways to register Activities with the Temporal Rust SDK.

## Table of Contents

1. [Raw API](#raw-api)
2. [Simplified API (Recommended)](#simplified-api-recommended)
3. [Macro-Based](#macro-based)
4. [Full Example](#full-example)
5. [Best Practices](#best-practices)

---

## Raw API

The SDK’s raw registration approach requires handling all details manually.

### When to Use

- Full control over serialization
- Non-JSON serialization formats
- Specialized error handling

### Code Example

```rust
use std::sync::Arc;
use std::pin::Pin;
use std::future::Future;
use std::collections::HashMap;
use temporalio_sdk::{ActContext, ActivityError, Worker};
use temporalio_common::protos::temporal::api::common::v1::Payload;

// 1. Define service (with dependencies)
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

// 2. Register Activity
let service = Arc::new(GreeterService { prefix: "Hello".to_string() });
let svc_clone = Arc::clone(&service);

worker.register_activity("greet-activity", move |_ctx: ActContext, payload: Payload| {
    let svc = Arc::clone(&svc_clone);
    
    Box::pin(async move {
        // Manual deserialize
        let input: GreetInput = match serde_json::from_slice(&payload.data) {
            Ok(i) => i,
            Err(e) => return Err(ActivityError::from(anyhow!("Deserialize failed: {}", e))),
        };

        // Call business method
        let output = svc.greet(input).await?;

        // Manual serialize
        let data = serde_json::to_vec(&output).map_err(|e| {
            ActivityError::from(anyhow!("Serialize failed: {}", e))
        })?;

        // Build Payload
        Ok(Payload {
            metadata: HashMap::from([(
                "encoding".to_string(),
                "json/plain".as_bytes().to_vec(),
            )]),
            data,
            ..Default::default()
        })
    }) as Pin<Box<dyn Future<Output = Result<Payload, ActivityError>> + Send>>
});
```

Approximate size: ~25 lines per Activity

---

## Simplified API (Recommended)

Use the `ActivityRegistrarJson` trait to automatically handle JSON serialization.

### When to Use

- JSON as the serialization format (most cases)
- Reduce boilerplate
- Compile-time type safety

### Code Example

```rust
use temporal_sdk_helpers::ActivityRegistrarJson;
use std::sync::Arc;

// 1. Define service (same as above)
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

// 2. Register Activity (only 3 lines!)
let service = Arc::new(GreeterService { prefix: "Hello".to_string() });
let svc = Arc::clone(&service);

worker.register_activity_json("greet-activity", move |_ctx, input: GreetInput| {
    let svc = Arc::clone(&svc);
    async move { svc.greet(input).await }
});
```

Size: 3 lines per Activity

### Advantages

- ✅ 88% less boilerplate
- ✅ Compile-time type safety
- ✅ Automatic JSON serialization
- ✅ Easy to maintain

---

## Macro-Based

Use the `register_activity!` macro to further simplify registration.

### When to Use

- Rapid prototyping
- Simple Activity registration
- No custom context handling

### Code Example

```rust
use temporal_sdk_helpers::register_activity;
use std::sync::Arc;

// 1. Define service
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
    
    async fn farewell(&self, input: GreetInput) -> Result<GreetOutput, ActivityError> {
        Ok(GreetOutput {
            message: format!("Goodbye, {}! {}", input.name, self.prefix),
        })
    }
}

// 2. Register multiple Activities (one per line)
let service = Arc::new(GreeterService { prefix: "Hello".to_string() });

register_activity!(worker, "greet", service, greet);
register_activity!(worker, "farewell", service, farewell);
```

Size: 1 line per Activity

### Macro Definition

```rust
#[macro_export]
macro_rules! register_activity {
    ($worker:expr, $activity_type:expr, $service:expr, $method:ident) => {{
        let svc = std::sync::Arc::clone(&$service);
        $worker.register_activity_json($activity_type, move |ctx, input| {
            let svc = std::sync::Arc::clone(&svc);
            async move { svc.$method(input).await }
        });
    }};
}
```

---

## Full Example

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use temporal_sdk_helpers::ActivityRegistrarJson;
use temporalio_sdk::{ActContext, ActivityError, WfContext, WfExitValue, Worker, WorkflowResult};
use temporalio_sdk_core::init_worker;

// ============ Data Types ============

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GreetInput {
    name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GreetOutput {
    message: String,
}

// ============ Service Definition ============

#[derive(Clone)]
struct GreeterService {
    prefix: String,
}

impl GreeterService {
    fn new(prefix: impl Into<String>) -> Self {
        Self { prefix: prefix.into() }
    }

    async fn greet(&self, input: GreetInput) -> Result<GreetOutput, ActivityError> {
        Ok(GreetOutput {
            message: format!("{} {}!", self.prefix, input.name),
        })
    }

    async fn farewell(&self, input: GreetInput) -> Result<GreetOutput, ActivityError> {
        Ok(GreetOutput {
            message: format!("Goodbye, {}! {}", input.name, self.prefix),
        })
    }
}

// ============ Workflow ============

const TASK_QUEUE: &str = "greeting-q";
const GREET_ACTIVITY: &str = "greet";
const FAREWELL_ACTIVITY: &str = "farewell";

async fn greeting_workflow(ctx: WfContext) -> WorkflowResult<String> {
    use temporalio_sdk::ActivityOptions;
    use std::time::Duration;
    
    // Build input
    let input = GreetInput { name: "World".to_string() };
    
    // Invoke greet activity
    let greet_opts = ActivityOptions {
        activity_type: GREET_ACTIVITY.to_string(),
        start_to_close_timeout: Some(Duration::from_secs(5)),
        input: examples_shared::json_payload(&input)?,
        ..Default::default()
    };
    
    let greet_res = ctx.activity(greet_opts).await;
    
    // Handle result...
    Ok(WfExitValue::Normal("Done".to_string()))
}

// ============ Worker Startup ============

async fn run_worker() -> Result<()> {
    // Initialize service
    let service = Arc::new(GreeterService::new("Hello"));

    // Initialize Temporal Worker (connection code omitted)
    let mut worker = Worker::new_from_core(..., TASK_QUEUE);

    // Register Activities using the simplified API
    let svc = Arc::clone(&service);
    worker.register_activity_json(GREET_ACTIVITY, move |_ctx, input: GreetInput| {
        let svc = Arc::clone(&svc);
        async move { svc.greet(input).await }
    });

    let svc = Arc::clone(&service);
    worker.register_activity_json(FAREWELL_ACTIVITY, move |_ctx, input: GreetInput| {
        let svc = Arc::clone(&svc);
        async move { svc.farewell(input).await }
    });

    // Register Workflow
    worker.register_wf("greeting-workflow", greeting_workflow);

    // Start Worker
    worker.run().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    run_worker().await
}
```

---

## Best Practices

### 1. Choose the right registration approach

| Scenario | Recommended | Reason |
|------|---------|------|
| Standard JSON serialization | Simplified API | Minimal code, type-safe |
| Rapid prototyping | Macro-Based | Fastest to develop |
| Custom serialization | Raw API | Full control |
| Complex dependency injection | Simplified API | Clear dependency handling |

### 2. Service design recommendations

```rust
// ✅ Recommended: Services hold dependencies as fields
#[derive(Clone)]
struct OrderService {
    db: Arc<Database>,
    payment_client: Arc<PaymentClient>,
    config: Config,
}

impl OrderService {
    async fn create_order(&self, input: CreateOrderInput) -> Result<Order, ActivityError> {
        // Use self.db, self.payment_client, etc.
    }
}

// ❌ Not recommended: global state
static DB: Lazy<Database> = Lazy::new(|| Database::new());
```

### 3. Error handling

```rust
// ✅ Recommended: use ActivityError
async fn process(&self, input: Input) -> Result<Output, ActivityError> {
    // Retryable error
    if network_failed {
        return Err(anyhow!("Network error").into()); // Retries by default
    }
    
    // Non-retryable error
    if invalid_input {
        return Err(ActivityError::NonRetryable(anyhow!("Invalid input")));
    }
    
    Ok(output)
}
```

### 4. Input/Output types

```rust
// ✅ Recommended: use explicit input/output types
#[derive(Serialize, Deserialize)]
struct CreateOrderInput {
    user_id: String,
    items: Vec<LineItem>,
    shipping_address: Address,
}

#[derive(Serialize, Deserialize)]
struct CreateOrderOutput {
    order_id: String,
    total: Money,
    status: OrderStatus,
}

// ❌ Not recommended: use generic types
async fn process(&self, json: String) -> Result<String, ActivityError> {
    // Loses type safety
}
```

### 5. Registering multiple Activities

```rust
// Recommended: register centrally
fn register_activities(worker: &mut Worker, service: Arc<MyService>) {
    // Order-related
    let svc = Arc::clone(&service);
    worker.register_activity_json("create-order", move |ctx, input| {
        let svc = Arc::clone(&svc);
        async move { svc.create_order(input).await }
    });

    let svc = Arc::clone(&service);
    worker.register_activity_json("cancel-order", move |ctx, input| {
        let svc = Arc::clone(&svc);
        async move { svc.cancel_order(input).await }
    });

    // Payment-related
    let svc = Arc::clone(&service);
    worker.register_activity_json("process-payment", move |ctx, input| {
        let svc = Arc::clone(&svc);
        async move { svc.process_payment(input).await }
    });
}
```

---

## Summary

| Approach | Lines of Code | Use Case | Recommendation |
|------|-------|---------|-------|
| Raw API | ~25 lines | Full control | ⭐⭐ |
| Simplified API | 3 lines | Standard cases | ⭐⭐⭐⭐⭐ |
| Macro-Based | 1 line | Rapid development | ⭐⭐⭐⭐ |

**Core API:**

```rust
use temporal_sdk_helpers::ActivityRegistrarJson;

let svc = Arc::clone(&service);
worker.register_activity_json("activity-name", move |ctx, input: InputType| {
    let svc = Arc::clone(&svc);
    async move { svc.method(input).await }
});
```

**Advantages:**
- 88% less boilerplate
- Compile-time type safety
- Automatic JSON serialization
- Dependency injection support
