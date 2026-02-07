# Activity Registration Guide

Use this guide to choose the smallest safe registration pattern for the Temporal Rust SDK prototype.

## Quick decision matrix

| Option | Choose when | Typical code size | Recommendation |
| --- | --- | --- | --- |
| register_activity_json | Input and output are JSON and strongly typed | Small | Default |
| register_activity macro | You want one line registration for struct methods | Smallest | Use for quick prototypes |
| raw register_activity | You need custom payload format or low level metadata control | Largest | Use only when required |

## Fast selection steps

1. Start with register_activity_json from src/lib.rs.
2. Switch to the macro only if the handler is a direct struct method and you want shorter setup.
3. Move to raw register_activity only when JSON helpers cannot represent your payload or metadata requirements.

## Minimal patterns

### Pattern A: register_activity_json (default)

~~~rust
use std::sync::Arc;
use temporal_sdk_helpers::ActivityRegistrarJson;

#[derive(Clone)]
struct GreeterService {
    prefix: String,
}

#[derive(serde::Deserialize)]
struct GreetInput {
    name: String,
}

#[derive(serde::Serialize)]
struct GreetOutput {
    message: String,
}

impl GreeterService {
    async fn greet(
        &self,
        input: GreetInput,
    ) -> Result<GreetOutput, temporalio_sdk::ActivityError> {
        Ok(GreetOutput {
            message: format!("{} {}", self.prefix, input.name),
        })
    }
}

let service = Arc::new(GreeterService { prefix: "Hello".to_string() });
let svc = Arc::clone(&service);
worker.register_activity_json("greet", move |_ctx, input: GreetInput| {
    let svc = Arc::clone(&svc);
    async move { svc.greet(input).await }
});
~~~

### Pattern B: register_activity macro

~~~rust
use std::sync::Arc;
use temporal_sdk_helpers::{register_activity, ActivityRegistrarJson};

let service = Arc::new(GreeterService { prefix: "Hello".to_string() });
register_activity!(worker, "greet", service, greet);
~~~

### Pattern C: raw register_activity

~~~rust
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use temporalio_common::protos::temporal::api::common::v1::Payload;
use temporalio_sdk::{ActContext, ActivityError};

worker.register_activity("greet", move |_ctx: ActContext, payload: Payload| {
    Box::pin(async move {
        let input: GreetInput = serde_json::from_slice(&payload.data)
            .map_err(|err| ActivityError::from(anyhow::anyhow!("decode error: {}", err)))?;

        let output = GreetOutput {
            message: format!("Hello {}", input.name),
        };

        let data = serde_json::to_vec(&output)
            .map_err(|err| ActivityError::from(anyhow::anyhow!("encode error: {}", err)))?;

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
~~~

## Rules that prevent most bugs

- Keep business side effects inside activities, not workflow code.
- Use serde compatible structs for helper based registration.
- Keep payload encoding metadata aligned with actual payload format.
- Return ActivityError with clear context for decode and encode failures.
- Register activity names as stable API contracts.

## Verification checklist

1. Build example package with cargo check.
2. Start worker and verify registration logs include expected activity names.
3. Run starter command and verify workflow result payload can be decoded.
4. For raw registration, test one malformed payload path and verify error clarity.

## Related files

- reference/ACTIVITY.md
- src/lib.rs
- examples/struct-activity
