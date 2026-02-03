---
name: temporal-rust-sdk
description: Provides verified examples and implementation patterns for the Temporal Rust SDK (Prototype). Use this skill to learn how to build Workflows, Activities, Sagas, and Signal processing applications using Rust.
license: MIT
metadata:
  language: rust
  framework: temporal
  status: prototype
---

# Temporal Rust SDK Skill

> ## Documentation Index
> - [Client Reference](reference/CLIENT.md)
> - [Worker Reference](reference/WORKER.md)
> - [Workflow Reference](reference/WORKFLOW.md)
> - [Activity Reference](reference/ACTIVITY.md)

This skill contains a collection of verified examples demonstrating how to use the **Temporal Rust SDK** (currently based on `temporalio-sdk-core`).

Since the Rust SDK is in a pre-alpha state, these examples serve as a reference for handling common patterns like Sagas, Batching, and basic Workflow/Activity execution.

## Available Examples

The examples are located in the `examples/` directory and are organized as a Cargo workspace.

### 1. Hello World (`helloworld`)
A fundamental example showing:
- How to define and register an Activity.
- How to write a Workflow that executes an Activity.
- How to start a Workflow and poll for its result.

### 2. Batch Sliding Window (`batch-sliding-window`)
Demonstrates advanced Signal handling:
- Aggregates high-frequency signals into batches (size 10) or processes them upon timeout (5s).
- Uses `tokio::select!` to manage the race condition between Signals and Timers.
- Shows how to maintain local state within a Workflow.

### 3. Saga Transaction (`saga`)
Implements the Saga pattern for distributed transactions:
- Executes a sequence of steps: Reserve Car -> Reserve Hotel -> Reserve Flight.
- Simulates a failure in the final step (Flight).
- Triggers a **Compensation** phase to undo previous actions (Cancel Hotel -> Cancel Car) in reverse order.
- Demonstrates how to return structured results from a Workflow.

## Usage Guide

### Prerequisites
1.  **Rust Toolchain**: Install via `rustup`.
2.  **Protobuf Compiler**: `protoc` must be in your `PATH`.
3.  **Temporal Server**: Running locally at `localhost:7233`.

### Running an Example

Navigate to the example directory:
```bash
cd examples
```

**Step 1: Start the Worker**
```bash
# Replace <package> with: helloworld, batch-sliding-window, or saga
cargo run -p <package> -- worker
```

**Step 2: Run the Starter**
```bash
cargo run -p <package> -- starter
```

## Implementation Details

-   **Client Identity**: The prototype SDK requires setting `identity` in `ClientOptions`.
-   **Result Polling**: The SDK does not yet have a blocking `get_result` helper. These examples implement a polling loop using `get_workflow_execution_history` to fetch the final result.
-   **Workflow IDs**: Uses `uuid` to generate unique Workflow IDs for every run to avoid "Workflow execution already running" errors.
