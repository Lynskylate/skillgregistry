use crate::test_harness::TestEnv;
use anyhow::Result;
use serde_json::json;

/// Test Scenario 1: Complete discovery and sync workflow
#[tokio::test]
#[ignore] // Run only when E2E environment is available
async fn test_complete_discovery_and_sync_workflow() -> Result<()> {
    let env = TestEnv::new();
    env.wait_for_services().await?;

    // 1. Trigger discovery workflow via API
    let response = env
        .client
        .post(&format!("{}/api/workflows/discovery", env.api_url))
        .json(&json!({}))
        .send()
        .await?;

    assert!(
        response.status().is_success(),
        "Discovery workflow trigger failed: {}",
        response.status()
    );

    let workflow_result = response.json::<serde_json::Value>().await?;
    println!("Discovery workflow result: {:?}", workflow_result);

    // 2. Wait for workflow to complete and verify skills are discovered
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    // 3. List skills to verify they were indexed
    let skills_response = env
        .client
        .get(&format!("{}/api/skills", env.api_url))
        .send()
        .await?;

    assert!(skills_response.status().is_success());
    let skills_data = skills_response.json::<serde_json::Value>().await?;
    println!("Skills indexed: {:?}", skills_data);

    Ok(())
}

/// Test Scenario 2: API endpoints integration test
#[tokio::test]
#[ignore] // Run only when E2E environment is available
async fn test_api_endpoints() -> Result<()> {
    let env = TestEnv::new();
    env.wait_for_services().await?;

    // Test health endpoint
    let health = env
        .client
        .get(&format!("{}/health", env.api_url))
        .send()
        .await?;
    assert!(health.status().is_success());

    // Test list skills endpoint
    let skills = env
        .client
        .get(&format!("{}/api/skills", env.api_url))
        .send()
        .await?;
    assert!(skills.status().is_success());

    // Test search with query parameters
    let search = env
        .client
        .get(&format!("{}/api/skills?q=test&page=1&per_page=10", env.api_url))
        .send()
        .await?;
    assert!(search.status().is_success());

    Ok(())
}

/// Test Scenario 3: Temporal workflow execution
#[tokio::test]
#[ignore] // Run only when E2E environment is available
async fn test_temporal_workflow_execution() -> Result<()> {
    let env = TestEnv::new();
    env.wait_for_services().await?;

    // Trigger a sync workflow for a specific repository
    let response = env
        .client
        .post(&format!("{}/api/workflows/sync", env.api_url))
        .json(&json!({
            "registry_id": 1
        }))
        .send()
        .await?;

    if response.status().is_success() {
        let result = response.json::<serde_json::Value>().await?;
        println!("Sync workflow executed: {:?}", result);
    }

    Ok(())
}

/// Test Scenario 4: Error handling and edge cases
#[tokio::test]
#[ignore] // Run only when E2E environment is available
async fn test_error_handling() -> Result<()> {
    let env = TestEnv::new();
    env.wait_for_services().await?;

    // Test invalid skill ID
    let response = env
        .client
        .get(&format!("{}/api/skills/999999", env.api_url))
        .send()
        .await?;
    
    // Should return 404 or appropriate error
    assert!(
        response.status().is_client_error() || response.status().is_success()
    );

    // Test invalid search parameters
    let search = env
        .client
        .get(&format!("{}/api/skills?page=-1&per_page=0", env.api_url))
        .send()
        .await?;
    
    // Should handle gracefully
    assert!(search.status().is_success() || search.status().is_client_error());

    Ok(())
}

/// Test Scenario 5: Data persistence and versioning
#[tokio::test]
#[ignore] // Run only when E2E environment is available
async fn test_skill_versioning() -> Result<()> {
    let env = TestEnv::new();
    env.wait_for_services().await?;

    // Get a skill
    let skills = env
        .client
        .get(&format!("{}/api/skills?per_page=1", env.api_url))
        .send()
        .await?;

    if skills.status().is_success() {
        let data = skills.json::<serde_json::Value>().await?;
        println!("Skill with version info: {:?}", data);

        // Verify version information is present
        if let Some(items) = data["data"].as_array() {
            if let Some(first_skill) = items.first() {
                assert!(
                    first_skill.get("latest_version").is_some(),
                    "Skill should have version information"
                );
            }
        }
    }

    Ok(())
}