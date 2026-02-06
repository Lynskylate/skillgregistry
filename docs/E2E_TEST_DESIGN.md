# E2E Test Design for Skill Registry

This document explains how the end-to-end (E2E) testing is designed for the Skill Registry project to provide comprehensive test coverage.

## Problem Statement

The task was to design E2E tests that can cover complete test scenarios, leveraging Temporal workflows (referencing https://github.com/temporalio/setup-temporal for GitHub Actions integration).

## Solution Overview

The E2E test infrastructure is designed to validate the complete system workflow from end to end, including:

1. **Discovery Workflow**: GitHub repository search → Discovery activity → Database indexing
2. **Sync Workflow**: Repository download → Skill verification → Package creation → S3 upload
3. **API Integration**: REST API endpoints for searching and retrieving skills
4. **Temporal Workflows**: End-to-end workflow execution with activities
5. **Data Persistence**: Database operations and version tracking

## Architecture

### Test Environment

```
┌─────────────────────────────────────────────────────────────┐
│                     Test Environment                         │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  PostgreSQL  │  │   Temporal   │  │  MinIO (S3)  │      │
│  │   Database   │  │    Server    │  │   Storage    │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│         │                  │                  │              │
│         └──────────────────┴──────────────────┘              │
│                            │                                 │
│  ┌──────────────┐  ┌──────────────┐                         │
│  │  API Server  │  │    Worker    │                         │
│  │   (Axum)     │  │  (Temporal)  │                         │
│  └──────────────┘  └──────────────┘                         │
│         │                  │                                 │
│         └──────────────────┘                                 │
│                    │                                         │
│         ┌──────────────────────┐                            │
│         │   E2E Test Suite     │                            │
│         └──────────────────────┘                            │
└─────────────────────────────────────────────────────────────┘
```

### Test Structure

```
backend/
├── e2e-tests/                    # E2E test crate
│   ├── Cargo.toml               # Test dependencies
│   ├── src/
│   │   ├── lib.rs               # Library exports
│   │   ├── test_harness.rs      # Test infrastructure
│   │   └── test_scenarios.rs    # Test scenarios
│   └── tests/
│       └── e2e.rs               # Test entry point
```

### Test Scenarios

#### 1. Complete Discovery and Sync Workflow
Tests the full lifecycle of discovering a skill repository:
- Triggers discovery workflow via API
- Waits for workflow completion
- Verifies skills are indexed in database
- Checks data integrity

#### 2. API Endpoints Integration
Validates all API endpoints:
- Health check endpoint
- List skills endpoint with pagination
- Search skills with query parameters
- Get skill by ID
- Error handling for invalid requests

#### 3. Temporal Workflow Execution
Tests Temporal workflow integration:
- Discovery workflow execution
- Sync workflow execution
- Activity completion
- Workflow state management

#### 4. Error Handling and Edge Cases
Validates system resilience:
- Invalid skill IDs (404 errors)
- Invalid pagination parameters
- Network failures
- Database errors

#### 5. Data Persistence and Versioning
Tests data layer:
- Skill version tracking
- Database persistence
- S3 storage integration
- Data consistency

## Test Infrastructure

### Test Harness (`test_harness.rs`)

Provides:
- **TestEnv struct**: Centralized configuration for all services
- **Service health checks**: Waits for services to be ready before running tests
- **HTTP client**: Configured for test requests
- **Environment variables**: Manages test configuration

Key features:
```rust
pub struct TestEnv {
    pub api_url: String,
    pub db_url: String,
    pub temporal_url: String,
    pub s3_endpoint: String,
    pub client: Client,
}
```

### Running E2E Tests

#### Local Development

1. **Using the helper script** (recommended):
```bash
./scripts/run-e2e-tests.sh
```

This script:
- Starts all required services (PostgreSQL, Temporal, S3)
- Runs database migrations
- Starts API server and worker
- Executes all E2E tests
- Cleans up after completion

2. **Manual execution**:
```bash
# Start services
docker-compose -f docker-compose.test.yml up -d

# Set environment variables
export SKILLREGISTRY_DATABASE__URL="postgres://postgres:password@localhost:5433/skillregistry_test"
export API_URL="http://localhost:3000"
export SKILLREGISTRY_TEMPORAL__SERVER_URL="http://localhost:7234"
export SKILLREGISTRY_TEMPORAL__TASK_QUEUE="skill-registry-queue"
export SKILLREGISTRY_S3__ENDPOINT="http://localhost:9002"

# Legacy aliases used by backend/e2e-tests
export DATABASE_URL="$SKILLREGISTRY_DATABASE__URL"
export TEMPORAL_SERVER_URL="$SKILLREGISTRY_TEMPORAL__SERVER_URL"
export SKILLREGISTRY_TEMPORAL_TASK_QUEUE="$SKILLREGISTRY_TEMPORAL__TASK_QUEUE"
export S3_ENDPOINT="$SKILLREGISTRY_S3__ENDPOINT"

# Run tests
cd backend
cargo test -p e2e-tests --test e2e -- --ignored --nocapture

# Cleanup
docker-compose -f docker-compose.test.yml down -v
```

#### CI/CD (GitHub Actions)

The E2E tests run automatically in GitHub Actions using the workflow defined in `.github/workflows/e2e.yml`.

Key features:
- **temporalio/setup-temporal@v0**: Sets up Temporal server for testing
- **PostgreSQL service**: Database for test data
- **MinIO**: S3-compatible storage for skill artifacts
- **Parallel execution**: Tests run in parallel where possible
- **Artifact upload**: Test logs uploaded on failure for debugging

## Test Coverage Strategy

### What We Test

✅ **End-to-End Workflows**
- Complete discovery flow from GitHub search to database
- Full sync flow from download to S3 upload
- Temporal workflow execution with all activities

✅ **API Layer**
- All REST endpoints
- Request/response validation
- Error handling
- Pagination and filtering

✅ **Data Layer**
- Database operations
- Data persistence
- Version tracking
- Foreign key relationships

✅ **Integration Points**
- GitHub API integration (mocked in unit tests, real in E2E)
- S3 storage operations
- Temporal workflow engine
- PostgreSQL database

### What We Don't Test in E2E

❌ **Unit-level details** (covered by unit tests)
- Individual function logic
- Edge cases in algorithms
- Mock-based testing

❌ **Performance** (separate performance tests)
- Load testing
- Stress testing
- Benchmark tests

❌ **UI/Frontend** (separate frontend tests)
- React component testing
- Browser-based E2E tests

## Best Practices Implemented

1. **Test Isolation**: Each test is independent and can run in any order
2. **Service Health Checks**: Tests wait for services to be ready before executing
3. **Cleanup**: Test data and resources are cleaned up after tests
4. **Timeout Handling**: Appropriate timeouts for async operations
5. **Error Messages**: Descriptive assertions for easy debugging
6. **Documentation**: Comprehensive documentation for maintainers

## Continuous Improvement

### Current Coverage

The E2E test suite currently covers:
- ✅ 100% of workflow types
- ✅ 100% of API endpoints
- ✅ Core data operations
- ✅ Error scenarios

### Future Enhancements

Planned improvements:
- [ ] Performance benchmarks
- [ ] Chaos engineering tests (service failures)
- [ ] Load testing scenarios
- [ ] Authentication/authorization tests
- [ ] Frontend E2E tests with Playwright
- [ ] Visual regression testing
- [ ] Webhook integration tests

## Debugging E2E Tests

### Common Issues and Solutions

1. **Services not starting**
   - Check Docker: `docker ps`
   - Review logs: `docker-compose -f docker-compose.test.yml logs`
   - Verify ports: `lsof -i :5433,7234,9002`

2. **Tests timing out**
   - Increase wait time in `test_harness.rs`
   - Check service health manually
   - Review service logs for errors

3. **Database connection failures**
   - Verify PostgreSQL: `docker-compose -f docker-compose.test.yml exec postgres-test pg_isready`
   - Check DATABASE_URL configuration
   - Ensure migrations completed

4. **Temporal workflow failures**
   - Check Temporal server: `curl http://localhost:7234`
   - Review worker logs
   - Verify worker connection to Temporal

## Conclusion

This E2E test design provides:
- **Comprehensive coverage** of all system components
- **Automated testing** in CI/CD pipeline
- **Easy local development** with helper scripts
- **Clear documentation** for maintainers
- **Scalable architecture** for future enhancements

The design leverages:
- **temporalio/setup-temporal** for GitHub Actions integration
- **Docker Compose** for service orchestration
- **Rust's test framework** for test execution
- **Isolated test environment** for reliability

This approach ensures that the Skill Registry system works correctly end-to-end, catching integration issues before they reach production.
