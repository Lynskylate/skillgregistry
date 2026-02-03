# Temporal Rust SDK: Client Reference

This document describes how to use the `temporalio_client` to interact with the Temporal Server.

## 1. Connecting to Temporal

To connect to a Temporal server, use `ClientOptions`.

> **Note**: The `identity` field is **required** in the prototype SDK.

```rust
use temporalio_client::ClientOptions;
use temporalio_sdk_core::Url;
use std::str::FromStr;

let client_options = ClientOptions::builder()
    .target_url(Url::from_str("http://localhost:7233")?)
    .client_name("my-client")
    .client_version("0.1.0")
    .identity("my-client-identity".to_string()) // Required
    .build();

let client = client_options.connect("default", None).await?;
```

## 2. Starting a Workflow

Use `start_workflow` to initiate a workflow execution. It is recommended to generate a UUID for the `workflow_id` to prevent deduplication errors.

```rust
use temporalio_client::WorkflowOptions;
use temporalio_common::protos::temporal::api::enums::v1::WorkflowIdReusePolicy;

let wf_id = uuid::Uuid::new_v4().to_string();
let task_queue = "my-task-queue";
let workflow_type = "my-workflow-type";

// Construct payload (manual JSON serialization)
let payload = temporalio_common::protos::temporal::api::common::v1::Payload {
    metadata: std::collections::HashMap::from([(
        "encoding".to_string(), 
        "json/plain".as_bytes().to_vec()
    )]),
    data: serde_json::to_vec(&"my-input-arg").unwrap(),
    ..Default::default()
};

let res = client.start_workflow(
    vec![payload],
    task_queue.to_string(),
    wf_id.clone(),
    workflow_type.to_string(),
    None, // request_id
    WorkflowOptions {
        id_reuse_policy: WorkflowIdReusePolicy::AllowDuplicate,
        ..Default::default()
    },
).await?;

println!("Workflow started with Run ID: {}", res.run_id);
```

## 3. Signaling a Workflow

To send a signal to a running workflow:

```rust
let signal_name = "my-signal";
let signal_input = "some-data";

let payload = temporalio_common::protos::temporal::api::common::v1::Payload {
    metadata: std::collections::HashMap::from([("encoding".to_string(), "json/plain".as_bytes().to_vec())]),
    data: serde_json::to_vec(&signal_input).unwrap(),
    ..Default::default()
};

client.signal_workflow_execution(
    wf_id,
    "".to_string(), // run_id (optional if workflow_id is unique enough)
    signal_name.to_string(),
    Some(temporalio_common::protos::temporal::api::common::v1::Payloads { payloads: vec![payload] }),
    None
).await?;
```

## 4. Getting Workflow Results

The prototype SDK does not have a blocking `get_result` helper. You must poll the workflow history to find the completion event.

```rust
use temporalio_common::protos::temporal::api::enums::v1::EventType;
use std::time::Duration;

loop {
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    let history_response = client.get_workflow_execution_history(
        wf_id.clone(), 
        Some(run_id.clone()), 
        vec![]
    ).await?;

    if let Some(history) = history_response.history {
        for event in history.events {
            if event.event_type == EventType::WorkflowExecutionCompleted as i32 {
                // Extract result payload
                // ... (See helloworld example for full extraction logic)
                return Ok(());
            } else if event.event_type == EventType::WorkflowExecutionFailed as i32 {
                return Err(anyhow!("Workflow failed"));
            }
        }
    }
}
```
