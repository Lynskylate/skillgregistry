# Backend Architecture Note (2026-02)

This note summarizes the backend refactor focused on maintainability and low-risk internal optimization.

## What changed

- **API auth module decomposition**
  - `backend/api/src/auth.rs` now keeps handler flow logic while DTOs and extraction concerns are separated into:
    - `backend/api/src/auth/dto.rs`
    - `backend/api/src/auth/extractor.rs`
- **Origin allowlist caching**
  - Parsed frontend origins are computed once at API startup and reused by both CORS setup and refresh-origin checks.
- **Worker bootstrap modularization**
  - Worker startup was split into focused modules:
    - `backend/worker/src/bootstrap/context.rs`
    - `backend/worker/src/bootstrap/temporal.rs`
    - `backend/worker/src/bootstrap/register.rs`
  - `backend/worker/src/main.rs` is now a thin orchestration entrypoint.
- **Temporal contract centralization**
  - Activity/workflow names are defined in `backend/worker/src/contracts.rs` and reused across registration + workflow invocation.
- **Workflow helper consolidation**
  - Shared batch execution and status decoding helpers are in `backend/worker/src/workflows/mod.rs` to remove duplicated chunking logic.
- **GitHub HTTP infrastructure deduplication**
  - Shared client/header/retry logic moved to `backend/common/src/infra/github_http.rs`.
  - Both `common` and `worker` GitHub clients now reuse this code path.
- **Panic-to-error hardening**
  - Several initialization and payload-serialization paths were converted from `expect/unwrap` style to propagated errors.

## Why

- Reduce file-level complexity in API and worker entrypoints.
- Remove duplicated infrastructure logic that can drift over time.
- Keep public API behavior stable while improving internal boundaries.
- Improve test ergonomics (including `cargo test --quiet` argument handling).

## Compatibility

- No HTTP route or response contract changes were introduced.
- No database schema or migration changes were introduced.
- Temporal workflow/activity names remain unchanged; they are now constantized.
