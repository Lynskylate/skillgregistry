# E2E Testing Guide

This document describes the End-to-End (E2E) testing setup for the Skill Registry project.

## Overview

The E2E tests validate the complete system workflow including:
- Discovery workflow (GitHub repository search and indexing)
- Sync workflow (Download, verify, package, and upload skills)
- API endpoints (REST API for searching and retrieving skills)
- Temporal workflow execution
- Database persistence
- S3 storage integration

## Architecture

The E2E test infrastructure consists of:

1. **Test Services**:
   - PostgreSQL: Database for test data
   - Temporal: Workflow orchestration
   - MinIO/RustFS: S3-compatible object storage
   - API Server: REST API service
   - Worker: Background task processor

2. **Test Scenarios** (`backend/e2e-tests/src/test_scenarios.rs`):
   - Complete discovery and sync workflow
   - API endpoints integration
   - Temporal workflow execution
   - Error handling and edge cases
   - Data persistence and versioning

3. **Test Harness** (`backend/e2e-tests/src/test_harness.rs`):
   - Service health checks
   - Environment configuration
   - Utility functions

## Running E2E Tests Locally

### Prerequisites

- Docker and Docker Compose
- Rust (stable toolchain)
- PostgreSQL client tools (optional, for debugging)

### Quick Start

Use the provided script to run all E2E tests:

```bash
./scripts/run-e2e-tests.sh
```

This script will:
1. Start all required services using docker-compose
2. Run database migrations
3. Build and start the API server and worker
4. Execute all E2E tests
5. Clean up services after completion

### Manual Setup

If you prefer to run tests manually:

1. Start test infrastructure:
```bash
docker-compose -f docker-compose.test.yml up -d
```

2. Set environment variables:
```bash
export SKILLREGISTRY_DATABASE__URL="postgres://postgres:password@localhost:5433/skillregistry_test"
export API_URL="http://localhost:3000"
export SKILLREGISTRY_TEMPORAL__SERVER_URL="http://localhost:7234"
export SKILLREGISTRY_TEMPORAL__TASK_QUEUE="skill-registry-queue"
export SKILLREGISTRY_S3__ENDPOINT="http://localhost:9002"
export SKILLREGISTRY_S3__ACCESS_KEY_ID="rustfsadmin"
export SKILLREGISTRY_S3__SECRET_ACCESS_KEY="rustfsadmin"
export SKILLREGISTRY_S3__BUCKET="skills"
export SKILLREGISTRY_S3__REGION="us-east-1"

# Legacy aliases used by backend/e2e-tests
export DATABASE_URL="$SKILLREGISTRY_DATABASE__URL"
export TEMPORAL_SERVER_URL="$SKILLREGISTRY_TEMPORAL__SERVER_URL"
export SKILLREGISTRY_TEMPORAL_TASK_QUEUE="$SKILLREGISTRY_TEMPORAL__TASK_QUEUE"
export S3_ENDPOINT="$SKILLREGISTRY_S3__ENDPOINT"
export AWS_ACCESS_KEY_ID="$SKILLREGISTRY_S3__ACCESS_KEY_ID"
export AWS_SECRET_ACCESS_KEY="$SKILLREGISTRY_S3__SECRET_ACCESS_KEY"
export S3_BUCKET="$SKILLREGISTRY_S3__BUCKET"
export S3_REGION="$SKILLREGISTRY_S3__REGION"
```

3. Run migrations:
```bash
cd backend
cargo run --bin setup
```

4. Start services:
```bash
# Terminal 1: API Server
cargo run --bin api

# Terminal 2: Worker
cargo run --bin worker
```

5. Run tests:
```bash
# Terminal 3: E2E Tests
cargo test -p e2e-tests --test e2e -- --ignored --nocapture
```

6. Cleanup:
```bash
docker-compose -f docker-compose.test.yml down -v
```

## GitHub Actions CI/CD

The E2E tests run automatically in GitHub Actions on every push and pull request.

### Workflow Configuration

See `.github/workflows/e2e.yml` for the complete workflow configuration.

Key features:
- Uses `temporalio/setup-temporal@v0` action to set up Temporal server
- Runs PostgreSQL as a service
- Uses MinIO for S3-compatible storage
- Executes all E2E tests with detailed logging
- Uploads test logs on failure

### Triggering CI

The E2E workflow runs automatically on:
- Push to `main` or `master` branches
- Pull requests to `main` or `master` branches
- Manual trigger via `workflow_dispatch`

## Writing New E2E Tests

To add a new E2E test scenario:

1. Add a new test function in `backend/e2e-tests/src/test_scenarios.rs`:

```rust
#[tokio::test]
#[ignore] // Run only when E2E environment is available
async fn test_my_new_scenario() -> Result<()> {
    let env = TestEnv::new();
    env.wait_for_services().await?;
    
    // Your test logic here
    
    Ok(())
}
```

2. Use the `TestEnv` struct to access service URLs and HTTP client
3. Mark the test with `#[ignore]` so it only runs in E2E environment
4. Return `Result<()>` for proper error handling

## Test Coverage

The current E2E test suite covers:

- ✅ Discovery workflow execution
- ✅ Sync workflow execution
- ✅ API health check
- ✅ List skills endpoint
- ✅ Search skills with filters
- ✅ Get skill by ID
- ✅ Temporal workflow integration
- ✅ Error handling (404, invalid parameters)
- ✅ Data persistence and versioning

## Troubleshooting

### Services Not Starting

If services fail to start:
1. Check Docker is running: `docker ps`
2. Check logs: `docker-compose -f docker-compose.test.yml logs`
3. Verify ports are not in use: `lsof -i :5433,7234,9002`

### Tests Timing Out

If tests timeout waiting for services:
1. Increase wait time in `test_harness.rs`
2. Check service health: `docker-compose -f docker-compose.test.yml ps`
3. Review service logs for errors

### Database Connection Issues

If database connections fail:
1. Verify PostgreSQL is healthy: `docker-compose -f docker-compose.test.yml exec postgres-test pg_isready`
2. Check DATABASE_URL is correct
3. Ensure migrations ran successfully

### Temporal Workflow Failures

If Temporal workflows fail:
1. Check Temporal server is running: `curl http://localhost:7234`
2. Review worker logs for activity errors
3. Verify worker is connected to Temporal

## Best Practices

1. **Isolation**: Each test should be independent and not rely on other tests
2. **Cleanup**: Always clean up test data after tests complete
3. **Timeouts**: Set appropriate timeouts for async operations
4. **Error Messages**: Provide descriptive error messages in assertions
5. **Logging**: Use appropriate log levels (debug for verbose, info for key events)

## Future Improvements

Potential enhancements for the E2E test suite:

- [ ] Add performance benchmarks
- [ ] Test concurrent workflow execution
- [ ] Add chaos testing (service failures, network issues)
- [ ] Test authentication and authorization
- [ ] Add frontend E2E tests with Playwright/Cypress
- [ ] Implement test data fixtures for consistent scenarios
- [ ] Add visual regression testing for UI
- [ ] Test webhook integrations
- [ ] Add load testing scenarios

## References

- [Temporal Testing Documentation](https://docs.temporal.io/develop/go/testing)
- [setup-temporal GitHub Action](https://github.com/temporalio/setup-temporal)
- [Agent Skills Specification](https://agentskills.io/specification)
