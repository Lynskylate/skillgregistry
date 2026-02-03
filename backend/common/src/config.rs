use config::{Config, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub database: Option<DatabaseConfig>,
    pub s3: S3Config,
    pub github: GithubConfig,
    pub worker: WorkerConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    pub endpoint: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GithubConfig {
    pub search_keywords: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WorkerConfig {
    pub scan_interval_seconds: u64,
}

impl AppConfig {
    pub fn new() -> Result<Self, config::ConfigError> {
        let builder = Config::builder()
            // Default settings
            .set_default("s3.bucket", "skill-registry-bucket")?
            .set_default("s3.region", "us-east-1")?
            .set_default("github.search_keywords", "topic:agent-skill")?
            .set_default("worker.scan_interval_seconds", 3600)?
            // Config file
            .add_source(File::with_name("config.toml").required(false))
            .add_source(File::with_name("/etc/skillregistry/config.toml").required(false))
            // Environment variables: DATABASE_URL, S3__BUCKET, etc.
            .add_source(Environment::default().separator("__"));

        builder.build()?.try_deserialize()
    }
}
