---
name: temporal-rust-sdk
description: Provide implementation patterns and runnable examples for the Temporal Rust SDK prototype. Use when building, migrating, or debugging Temporal applications in Rust, including Workflow and Activity authoring, Worker and Client wiring, Signal handling, Local Activities, Saga compensation, and activity registration strategies.
---

# Temporal Rust SDK

Use this skill to produce working Rust code for the Temporal prototype SDK with minimal trial and error.

## Fast path

1. Confirm prerequisites in reference/README.md.
2. Select the closest runnable example under examples/.
3. Reuse the same worker and starter command flow.
4. Port business logic while keeping Temporal wiring patterns.
5. Run smoke checks before sharing final code.

## Reference router

- reference/CLIENT.md: client creation, namespace setup, workflow start, signals, result polling.
- reference/WORKER.md: worker setup, task queue binding, run loop details.
- reference/WORKFLOW.md: workflow structure, timers, signal loops, local activities, saga orchestration.
- reference/ACTIVITY.md: activity contracts, payload boundaries, error handling.
- reference/ACTIVITY_REGISTRATION.md: activity registration decision matrix and minimal templates.

Load only the reference file needed for the current task.

## Activity registration rule

1. Default to register_activity_json from src/lib.rs for typed JSON payloads.
2. Use register_activity macro for short struct method prototypes.
3. Use raw register_activity only when custom payload encoding or metadata control is required.

Keep request and response structs serde compatible when using helper based registration.

## Smoke check

Use the reusable smoke check script before finalizing updates:

~~~bash
cd .agent/skills/temporal-rust-sdk
./scripts/smoke_check.sh
~~~

Useful options:

~~~bash
./scripts/smoke_check.sh --all-packages --skip-runtime
./scripts/smoke_check.sh --package saga --runtime-package saga
./scripts/smoke_check.sh --dry-run
~~~

The script compiles selected example packages and runs a runtime smoke check when a Temporal server is reachable at 127.0.0.1:7233.

## Manual run commands

Run from .agent/skills/temporal-rust-sdk/examples:

~~~bash
cargo run -p helloworld -- worker
cargo run -p helloworld -- starter --name Alice
~~~

Package names: helloworld, batch-sliding-window, saga, localactivity, struct-activity.

## Implementation checklist

- Set a unique workflow ID for each run.
- Keep workflow logic deterministic and move side effects to activities.
- Set explicit activity and local activity timeout policies.
- Execute saga compensation in reverse order.
- Keep worker and starter logs for reproducibility.

## Common failure patterns

- Missing protoc causes build failures.
- Reused workflow IDs cause already running errors.
- Blocking calls inside workflows can stall task progress.
- Encoding metadata and payload format mismatches cause decode failures.
