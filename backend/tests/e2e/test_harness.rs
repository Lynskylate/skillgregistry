use anyhow::Result;
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;

/// Test environment configuration
pub struct TestEnv {
    pub api_url: String,
    pub db_url: String,
    pub temporal_url: String,
    pub s3_endpoint: String,
    pub client: Client,
}

impl TestEnv {
    /// Create a new test environment with default docker-compose URLs
    pub fn new() -> Self {
        Self {
            api_url: std::env::var("API_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            db_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:password@localhost:5432/skillregistry".to_string()
            }),
            temporal_url: std::env::var("TEMPORAL_SERVER_URL")
                .unwrap_or_else(|_| "http://localhost:7233".to_string()),
            s3_endpoint: std::env::var("S3_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:9000".to_string()),
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    /// Wait for all services to be ready
    pub async fn wait_for_services(&self) -> Result<()> {
        println!("Waiting for services to be ready...");

        // Wait for API
        self.wait_for_service(&format!("{}/health", self.api_url), "API")
            .await?;

        // Wait for Temporal
        self.wait_for_service(&self.temporal_url, "Temporal")
            .await?;

        println!("All services ready!");
        Ok(())
    }

    async fn wait_for_service(&self, url: &str, name: &str) -> Result<()> {
        let max_attempts = 30;
        let mut attempt = 0;

        while attempt < max_attempts {
            attempt += 1;
            match self.client.get(url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    println!("{} is ready", name);
                    return Ok(());
                }
                _ => {
                    if attempt < max_attempts {
                        println!(
                            "Waiting for {} (attempt {}/{})",
                            name, attempt, max_attempts
                        );
                        sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        }

        anyhow::bail!("{} failed to start after {} attempts", name, max_attempts)
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}
