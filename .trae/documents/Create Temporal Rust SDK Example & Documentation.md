I will create a **Temporal Rust SDK Example** based on the prototype SDK found in `temporalio/sdk-core`.

Since the Rust SDK is currently in a prototype/pre-alpha state (located in `crates/sdk` of the core repository), the implementation will require using the git dependency directly.

### 1. Project Structure Setup
I will create the following directory structure:
- `.agent/skills/temporal-rust-sdk/example/`: The Rust project containing the code.
- `.agent/skills/temporal-rust-sdk/reference/`: Documentation and references.

### 2. Implementation Steps
1.  **Dependency Configuration**: Create a `Cargo.toml` that references the `temporal-sdk` crate directly from the `temporalio/sdk-core` GitHub repository.
2.  **Workflow & Activity Definition**:
    - Implement a basic `Hello World` workflow using the Rust SDK's `#[workflow]` (or equivalent) attributes.
    - Implement a corresponding Activity.
3.  **Worker Implementation**:
    - Create a `worker` binary that connects to the Temporal server, registers the workflow/activity, and listens for tasks.
4.  **Starter Implementation**:
    - Create a `starter` binary (or subcommand) to trigger the workflow execution.

### 3. Documentation
- Create `.agent/skills/temporal-rust-sdk/reference/README.md` explaining:
    - Prerequisites (Local Temporal Server).
    - How to run the worker and starter.
    - Notes on the experimental nature of the Rust SDK.

**Note**: As the Rust SDK is experimental and lacks published documentation, I will perform a targeted search of the `crates/sdk` directory during the execution phase to ensure the syntax matches the current state of the prototype.