# Temporal Rust SDK Examples

This directory contains examples using the **prototype** Temporal Rust SDK.

> **⚠️ WARNING**: The Rust SDK is currently in a **pre-alpha** state (`temporalio/sdk-core`). The API is unstable, undocumented, and subject to change.

## Prerequisites

1.  **Rust**: Ensure you have Rust installed (`cargo`).
2.  **Protobuf Compiler (`protoc`)**: You must have `protoc` installed and in your `PATH`.
    -   Ubuntu: `sudo apt install protobuf-compiler`
    -   MacOS: `brew install protobuf`
    -   Manual: Download from [GitHub Releases](https://github.com/protocolbuffers/protobuf/releases), unzip, and add `bin/` to `PATH`.
3.  **Temporal Server**: You need a running Temporal Server.
    -   [Temporal CLI](https://docs.temporal.io/cli/): `temporal server start-dev`

## Documentation

-   [Client Reference](CLIENT.md): Connecting, starting workflows, signaling, and getting results.
-   [Worker Reference](WORKER.md): Configuring and running workers.
-   [Workflow Reference](WORKFLOW.md): Defining workflows, activities, timers, and signals.
-   [Activity Reference](ACTIVITY.md): Defining activities and error handling.

## Project Structure

The examples are organized as a Cargo workspace:

-   `helloworld/`: Basic Workflow and Activity implementation.
-   `batch-sliding-window/`: Demonstrates handling Signals, Timers (`loop` + `tokio::select!`), and batch processing.
-   `saga/`: Demonstrates the Saga pattern (Compensation) using a manual compensation stack.

## How to Run

Ensure your Temporal server is running at `localhost:7233`.

### 1. Hello World

**Worker:**
```bash
cargo run -p helloworld -- worker
```

**Starter:**
```bash
cargo run -p helloworld -- starter --name "Trae User"
```

### 2. Batch Sliding Window

This example aggregates signals into batches (size 10) or processes them after a timeout.

**Worker:**
```bash
cargo run -p batch-sliding-window -- worker
```

**Starter:**
```bash
cargo run -p batch-sliding-window -- starter
```
(The starter sends 25 signals rapidly).

### 3. Saga

This example demonstrates a distributed transaction with compensation. It attempts to reserve a Car, Hotel, and Flight. The Flight reservation is hardcoded to fail, triggering compensation for Hotel and Car.

**Worker:**
```bash
cargo run -p saga -- worker
```

**Starter:**
```bash
cargo run -p saga -- starter
```

## Implementation Notes

-   **Client Identity**: The prototype SDK requires an explicit `identity` field in `ClientOptions`.
-   **Async/Await**: Rust workflows use `async/await`. `tokio::select!` is used for waiting on multiple futures (like Timer vs Signal).
-   **Compensations**: Unlike Go's `defer`, Rust Sagas use an explicit `Vec` or struct to track and execute compensations in reverse order.
