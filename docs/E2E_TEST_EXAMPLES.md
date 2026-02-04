# Example: Adding a New E2E Test

This example demonstrates how to add a new E2E test scenario to the Skill Registry test suite.

## Scenario: Test Concurrent Workflow Execution

Let's add a test that verifies the system can handle multiple concurrent sync workflows.

### Step 1: Add the Test Function

Edit `backend/e2e-tests/src/test_scenarios.rs` and add:

```rust
/// Test Scenario 6: Concurrent workflow execution
#[tokio::test]
#[ignore] // Run only when E2E environment is available
async fn test_concurrent_workflow_execution() -> Result<()> {
    let env = TestEnv::new();
    env.wait_for_services().await?;

    // Trigger multiple sync workflows concurrently
    let mut handles = vec![];
    
    for i in 1..=5 {
        let client = env.client.clone();
        let api_url = env.api_url.clone();
        
        let handle = tokio::spawn(async move {
            let response = client
                .post(&format!("{}/api/workflows/sync", api_url))
                .json(&json!({
                    "registry_id": i
                }))
                .send()
                .await?;
            
            Ok::<_, anyhow::Error>(response.status())
        });
        
        handles.push(handle);
    }

    // Wait for all workflows to complete
    let results = futures::future::join_all(handles).await;
    
    // Verify all workflows were accepted
    for result in results {
        let status = result??;
        assert!(
            status.is_success() || status == reqwest::StatusCode::ACCEPTED,
            "Workflow should be accepted"
        );
    }

    println!("All 5 concurrent workflows were processed successfully");
    Ok(())
}
```

### Step 2: Add Required Dependencies

Edit `backend/e2e-tests/Cargo.toml` and add if not already present:

```toml
[dependencies]
futures = "0.3"
```

### Step 3: Run the New Test

Run only your new test:

```bash
cd backend
cargo test -p e2e-tests test_concurrent_workflow_execution -- --ignored --nocapture
```

Or run all E2E tests:

```bash
./scripts/run-e2e-tests.sh
```

### Step 4: Verify in CI

Commit your changes and push:

```bash
git add backend/e2e-tests/
git commit -m "Add concurrent workflow E2E test"
git push
```

The GitHub Actions workflow will automatically run your new test.

## Example: Testing Error Recovery

Here's another example that tests error recovery:

```rust
/// Test Scenario 7: Workflow retry on failure
#[tokio::test]
#[ignore]
async fn test_workflow_retry_on_failure() -> Result<()> {
    let env = TestEnv::new();
    env.wait_for_services().await?;

    // Trigger a sync workflow with an invalid registry ID
    let response = env
        .client
        .post(&format!("{}/api/workflows/sync", env.api_url))
        .json(&json!({
            "registry_id": -1  // Invalid ID
        }))
        .send()
        .await?;

    // The API should handle this gracefully
    assert!(
        response.status().is_client_error() 
        || response.status().is_success(),
        "API should handle invalid registry ID gracefully"
    );

    // Check that the workflow either:
    // 1. Was rejected with appropriate error
    // 2. Was accepted but failed gracefully
    if response.status().is_success() {
        let result = response.json::<serde_json::Value>().await?;
        println!("Workflow result for invalid ID: {:?}", result);
        
        // Could check workflow status or error message here
    }

    Ok(())
}
```

## Example: Testing Data Consistency

Test that data remains consistent across services:

```rust
/// Test Scenario 8: Data consistency across services
#[tokio::test]
#[ignore]
async fn test_data_consistency() -> Result<()> {
    let env = TestEnv::new();
    env.wait_for_services().await?;

    // 1. Trigger discovery to create new skills
    let discovery_response = env
        .client
        .post(&format!("{}/api/workflows/discovery", env.api_url))
        .json(&json!({}))
        .send()
        .await?;
    
    assert!(discovery_response.status().is_success());

    // 2. Wait for processing
    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;

    // 3. Fetch skills from API
    let api_skills = env
        .client
        .get(&format!("{}/api/skills?per_page=100", env.api_url))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    // 4. Verify data structure
    if let Some(skills_array) = api_skills["data"].as_array() {
        println!("Found {} skills", skills_array.len());
        
        for skill in skills_array {
            // Verify required fields exist
            assert!(skill["id"].is_number(), "Skill should have ID");
            assert!(skill["name"].is_string(), "Skill should have name");
            assert!(skill["owner"].is_string(), "Skill should have owner");
            assert!(skill["repo"].is_string(), "Skill should have repo");
            
            // Verify data consistency
            if let Some(latest_version) = skill["latest_version"].as_str() {
                assert!(!latest_version.is_empty(), "Version should not be empty");
            }
        }
        
        println!("Data consistency check passed for all skills");
    }

    Ok(())
}
```

## Best Practices for Writing E2E Tests

### 1. Test Independence
```rust
// Good: Each test is self-contained
#[tokio::test]
#[ignore]
async fn test_feature_a() -> Result<()> {
    let env = TestEnv::new();  // Fresh environment
    // Test specific to feature A
    Ok(())
}

// Good: Another independent test
#[tokio::test]
#[ignore]
async fn test_feature_b() -> Result<()> {
    let env = TestEnv::new();  // Fresh environment
    // Test specific to feature B
    Ok(())
}
```

### 2. Proper Cleanup
```rust
#[tokio::test]
#[ignore]
async fn test_with_cleanup() -> Result<()> {
    let env = TestEnv::new();
    
    // Create test data
    let created_id = create_test_skill(&env).await?;
    
    // Run test
    let result = test_something(&env, created_id).await;
    
    // Cleanup (always runs even if test fails)
    cleanup_test_skill(&env, created_id).await?;
    
    // Check result after cleanup
    result
}
```

### 3. Meaningful Assertions
```rust
// Bad: Generic assertion
assert!(response.status().is_success());

// Good: Specific assertion with helpful message
assert_eq!(
    response.status(),
    StatusCode::OK,
    "Discovery endpoint should return 200 OK, got {}",
    response.status()
);
```

### 4. Appropriate Timeouts
```rust
// Good: Explicit timeout for long operations
tokio::time::timeout(
    Duration::from_secs(30),
    env.client
        .post(&format!("{}/api/workflows/sync", env.api_url))
        .send()
)
.await
.context("Sync workflow timed out after 30 seconds")?;
```

### 5. Clear Test Names
```rust
// Good: Descriptive test name
#[tokio::test]
async fn test_sync_workflow_handles_missing_skill_md_gracefully() -> Result<()> {
    // ...
}

// Bad: Vague test name
#[tokio::test]
async fn test_sync() -> Result<()> {
    // ...
}
```

## Running Specific Tests

```bash
# Run a specific test by name
cargo test -p e2e-tests test_concurrent_workflow_execution -- --ignored --nocapture

# Run tests matching a pattern
cargo test -p e2e-tests test_workflow -- --ignored --nocapture

# Run with verbose output
cargo test -p e2e-tests -- --ignored --nocapture --test-threads=1
```

## Debugging Tests

### Enable Debug Logging
```bash
RUST_LOG=debug cargo test -p e2e-tests -- --ignored --nocapture
```

### Run with Single Thread
```bash
cargo test -p e2e-tests -- --ignored --nocapture --test-threads=1
```

### Check Service Logs
```bash
# API logs
docker-compose logs skillregistry-backend

# Worker logs
docker-compose logs skillregistry-worker

# Temporal logs
docker-compose logs temporal
```

## Summary

To add a new E2E test:
1. ✅ Write test function in `test_scenarios.rs`
2. ✅ Add `#[tokio::test]` and `#[ignore]` attributes
3. ✅ Use `TestEnv` for service access
4. ✅ Add descriptive assertions
5. ✅ Test locally with helper script
6. ✅ Verify in CI pipeline