use anyhow::{anyhow, bail, Context, Result};
use aws_config::BehaviorVersion;
use aws_credential_types::{provider::SharedCredentialsProvider, Credentials};
use aws_sdk_s3::Client as S3Client;
use chrono::{NaiveDateTime, Utc};
use common::entities::{
    plugin_versions, plugins,
    prelude::{PluginVersions, Plugins, SkillRegistry, SkillVersions, Skills},
    skill_registry, skill_versions, skills,
};
use sea_orm::{ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;
use temporalio_client::{ClientOptions, WorkflowClientTrait, WorkflowOptions};
use temporalio_common::protos::temporal::api::common::v1::Payload;
use temporalio_common::protos::temporal::api::enums::v1::WorkflowIdReusePolicy;
use temporalio_sdk_core::Url;
use tokio::time::{sleep, Instant};
use uuid::Uuid;

#[derive(Debug, Clone)]
struct E2eConfig {
    database_url: String,
    temporal_server_url: String,
    temporal_task_queue: String,
    discovery_query: String,
    target_owner: String,
    target_repo: String,
    discovery_timeout: Duration,
    sync_timeout: Duration,
    s3_bucket: String,
    s3_region: String,
    s3_endpoint: Option<String>,
    s3_force_path_style: bool,
    aws_access_key_id: Option<String>,
    aws_secret_access_key: Option<String>,
}

impl E2eConfig {
    fn from_env() -> Result<Self> {
        let github_token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
        if github_token.trim().is_empty() {
            bail!(
                "GITHUB_TOKEN is required. Configure it in your environment before running e2e tests."
            );
        }

        let discovery_query = env_or("E2E_DISCOVERY_QUERY", "repo:anthropics/skills");
        let parsed_target = parse_target_from_query(&discovery_query);

        let target_owner = std::env::var("E2E_TARGET_OWNER")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| parsed_target.as_ref().map(|(owner, _)| owner.to_string()))
            .ok_or_else(|| {
                anyhow!(
                    "Missing target owner. Set E2E_TARGET_OWNER or use a discovery query with repo:<owner>/<repo>."
                )
            })?;

        let target_repo = std::env::var("E2E_TARGET_REPO")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| parsed_target.as_ref().map(|(_, repo)| repo.to_string()))
            .ok_or_else(|| {
                anyhow!(
                    "Missing target repo. Set E2E_TARGET_REPO or use a discovery query with repo:<owner>/<repo>."
                )
            })?;

        Ok(Self {
            database_url: env_or(
                "DATABASE_URL",
                "sqlite:///tmp/skillregistry-e2e.db?mode=rwc",
            ),
            temporal_server_url: env_or("TEMPORAL_SERVER_URL", "http://localhost:7233"),
            temporal_task_queue: env_or(
                "SKILLREGISTRY_TEMPORAL_TASK_QUEUE",
                "skill-registry-queue",
            ),
            discovery_query,
            target_owner,
            target_repo,
            discovery_timeout: Duration::from_secs(env_u64("E2E_DISCOVERY_TIMEOUT_SECS", 240)),
            sync_timeout: Duration::from_secs(env_u64("E2E_SYNC_TIMEOUT_SECS", 480)),
            s3_bucket: env_or("S3_BUCKET", "skills"),
            s3_region: env_or("S3_REGION", "us-east-1"),
            s3_endpoint: std::env::var("S3_ENDPOINT")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            s3_force_path_style: env_bool("S3_FORCE_PATH_STYLE", true),
            aws_access_key_id: std::env::var("AWS_ACCESS_KEY_ID")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            aws_secret_access_key: std::env::var("AWS_SECRET_ACCESS_KEY")
                .ok()
                .filter(|v| !v.trim().is_empty()),
        })
    }
}

#[derive(Debug, Clone)]
struct UploadedArtifact {
    source: &'static str,
    name: String,
    version: String,
    s3_key: String,
    created_at: NaiveDateTime,
}

#[tokio::test]
#[ignore = "requires Temporal, RustFS/S3, SQLite, and a valid GITHUB_TOKEN"]
async fn test_discovery_sync_and_upload() -> Result<()> {
    let cfg = E2eConfig::from_env()?;

    println!(
        "Running E2E with query='{}', target={}/{}, db='{}'",
        cfg.discovery_query, cfg.target_owner, cfg.target_repo, cfg.database_url
    );

    let db = Database::connect(&cfg.database_url)
        .await
        .context("failed to connect to sqlite database")?;

    let temporal = connect_temporal(&cfg).await?;
    let wf_opts = WorkflowOptions {
        id_reuse_policy: WorkflowIdReusePolicy::AllowDuplicate,
        ..Default::default()
    };

    let discovery_workflow_id = format!("e2e-discovery-{}", Uuid::new_v4());
    temporal
        .start_workflow(
            vec![create_json_payload(&vec![cfg.discovery_query.clone()])],
            cfg.temporal_task_queue.clone(),
            discovery_workflow_id.clone(),
            "discovery_workflow".to_string(),
            None,
            wf_opts.clone(),
        )
        .await
        .with_context(|| {
            format!(
                "failed to start discovery workflow '{}' on task queue '{}'",
                discovery_workflow_id, cfg.temporal_task_queue
            )
        })?;

    println!("Started discovery workflow: {}", discovery_workflow_id);

    let repo = wait_for_discovered_repo(&db, &cfg).await?;
    println!(
        "Discovered repo id={} status={} repo_type={:?}",
        repo.id, repo.status, repo.repo_type
    );

    let sync_started_at = Utc::now().naive_utc();
    let sync_workflow_id = format!("e2e-sync-{}-{}", repo.id, Uuid::new_v4());

    temporal
        .start_workflow(
            vec![create_json_payload(&repo.id)],
            cfg.temporal_task_queue.clone(),
            sync_workflow_id.clone(),
            "sync_repo_workflow".to_string(),
            None,
            wf_opts,
        )
        .await
        .with_context(|| {
            format!(
                "failed to start sync workflow '{}' for registry_id={}",
                sync_workflow_id, repo.id
            )
        })?;

    println!("Started sync workflow: {}", sync_workflow_id);

    let artifact =
        wait_for_uploaded_artifact(&db, repo.id, sync_started_at, cfg.sync_timeout).await?;

    println!(
        "Found uploaded artifact source={} name={} version={} key={} created_at={}",
        artifact.source, artifact.name, artifact.version, artifact.s3_key, artifact.created_at
    );

    assert_s3_object_exists(&cfg, &artifact.s3_key).await?;

    Ok(())
}

async fn connect_temporal(
    cfg: &E2eConfig,
) -> Result<impl WorkflowClientTrait + Clone + Send + Sync + 'static> {
    let options = ClientOptions::builder()
        .target_url(Url::from_str(&cfg.temporal_server_url)?)
        .client_name("skillregistry-e2e-tests")
        .client_version("0.1.0")
        .identity(format!("skillregistry-e2e-{}", Uuid::new_v4()))
        .build();

    options.connect("default", None).await.with_context(|| {
        format!(
            "failed to connect to Temporal at {}",
            cfg.temporal_server_url
        )
    })
}

async fn wait_for_discovered_repo(
    db: &DatabaseConnection,
    cfg: &E2eConfig,
) -> Result<skill_registry::Model> {
    let started = Instant::now();
    loop {
        let found = SkillRegistry::find()
            .filter(skill_registry::Column::Owner.eq(cfg.target_owner.clone()))
            .filter(skill_registry::Column::Name.eq(cfg.target_repo.clone()))
            .one(db)
            .await?;

        if let Some(repo) = found {
            if repo.status == "blacklisted" {
                bail!(
                    "target repo {}/{} was discovered but blacklisted: {}",
                    cfg.target_owner,
                    cfg.target_repo,
                    repo.blacklist_reason
                        .unwrap_or_else(|| "unknown reason".to_string())
                );
            }
            return Ok(repo);
        }

        if started.elapsed() > cfg.discovery_timeout {
            bail!(
                "timed out waiting for discovered repo {}/{} (query='{}')",
                cfg.target_owner,
                cfg.target_repo,
                cfg.discovery_query
            );
        }

        sleep(Duration::from_secs(3)).await;
    }
}

async fn wait_for_uploaded_artifact(
    db: &DatabaseConnection,
    registry_id: i32,
    created_after: NaiveDateTime,
    timeout: Duration,
) -> Result<UploadedArtifact> {
    let started = Instant::now();
    loop {
        if let Some(artifact) = find_uploaded_artifact(db, registry_id, created_after).await? {
            return Ok(artifact);
        }

        if let Some(repo) = SkillRegistry::find_by_id(registry_id).one(db).await? {
            if repo.status == "blacklisted" {
                bail!(
                    "sync blacklisted repo id={} during e2e: {}",
                    registry_id,
                    repo.blacklist_reason
                        .unwrap_or_else(|| "unknown reason".to_string())
                );
            }
        }

        if started.elapsed() > timeout {
            bail!(
                "timed out waiting for uploaded artifact for registry_id={}",
                registry_id
            );
        }

        sleep(Duration::from_secs(3)).await;
    }
}

async fn find_uploaded_artifact(
    db: &DatabaseConnection,
    registry_id: i32,
    created_after: NaiveDateTime,
) -> Result<Option<UploadedArtifact>> {
    let skill_rows = Skills::find()
        .filter(skills::Column::SkillRegistryId.eq(registry_id))
        .all(db)
        .await?;

    for skill in skill_rows {
        if let Some(version) = SkillVersions::find()
            .filter(skill_versions::Column::SkillId.eq(skill.id))
            .filter(skill_versions::Column::CreatedAt.gte(created_after))
            .filter(skill_versions::Column::S3Key.is_not_null())
            .order_by_desc(skill_versions::Column::CreatedAt)
            .one(db)
            .await?
        {
            if let Some(s3_key) = version.s3_key {
                return Ok(Some(UploadedArtifact {
                    source: "skill",
                    name: skill.name,
                    version: version.version,
                    s3_key,
                    created_at: version.created_at,
                }));
            }
        }
    }

    let plugin_rows = Plugins::find()
        .filter(plugins::Column::SkillRegistryId.eq(registry_id))
        .all(db)
        .await?;

    for plugin in plugin_rows {
        if let Some(version) = PluginVersions::find()
            .filter(plugin_versions::Column::PluginId.eq(plugin.id))
            .filter(plugin_versions::Column::CreatedAt.gte(created_after))
            .filter(plugin_versions::Column::S3Key.is_not_null())
            .order_by_desc(plugin_versions::Column::CreatedAt)
            .one(db)
            .await?
        {
            if let Some(s3_key) = version.s3_key {
                return Ok(Some(UploadedArtifact {
                    source: "plugin",
                    name: plugin.name,
                    version: version.version,
                    s3_key,
                    created_at: version.created_at,
                }));
            }
        }
    }

    Ok(None)
}

async fn assert_s3_object_exists(cfg: &E2eConfig, key: &str) -> Result<()> {
    let client = build_s3_client(cfg).await;

    match client
        .head_object()
        .bucket(&cfg.s3_bucket)
        .key(key)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(head_err) => {
            let get = client
                .get_object()
                .bucket(&cfg.s3_bucket)
                .key(key)
                .send()
                .await;

            match get {
                Ok(obj) => {
                    let bytes = obj.body.collect().await?.into_bytes();
                    if bytes.is_empty() {
                        bail!("S3 object '{}' exists but is empty", key);
                    }
                    Ok(())
                }
                Err(get_err) => Err(anyhow!(
                    "S3 object '{}' not found via head/get. head_error='{}', get_error='{}'",
                    key,
                    head_err,
                    get_err
                )),
            }
        }
    }
}

async fn build_s3_client(cfg: &E2eConfig) -> S3Client {
    let mut loader = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_types::region::Region::new(cfg.s3_region.clone()));

    if let (Some(ak), Some(sk)) = (
        cfg.aws_access_key_id.clone(),
        cfg.aws_secret_access_key.clone(),
    ) {
        let creds = Credentials::new(ak, sk, None, None, "e2e-tests");
        loader = loader.credentials_provider(SharedCredentialsProvider::new(creds));
    }

    if let Some(endpoint) = cfg.s3_endpoint.as_deref() {
        loader = loader.endpoint_url(normalize_endpoint(endpoint));
    }

    let shared_config = loader.load().await;
    let force_path_style = cfg.s3_force_path_style || cfg.s3_endpoint.is_some();

    let s3_config = aws_sdk_s3::config::Builder::from(&shared_config)
        .force_path_style(force_path_style)
        .build();

    S3Client::from_conf(s3_config)
}

fn create_json_payload(input: &impl Serialize) -> Payload {
    Payload {
        metadata: HashMap::from([("encoding".to_string(), b"json/plain".to_vec())]),
        data: serde_json::to_vec(input).unwrap_or_default(),
        ..Default::default()
    }
}

fn parse_target_from_query(query: &str) -> Option<(String, String)> {
    for token in query.split_whitespace() {
        if let Some(repo_token) = token.strip_prefix("repo:") {
            let mut parts = repo_token.split('/');
            let owner = parts.next()?.trim();
            let repo = parts.next()?.trim();
            if !owner.is_empty() && !repo.is_empty() {
                return Some((owner.to_string(), repo.to_string()));
            }
        }
    }
    None
}

fn normalize_endpoint(endpoint: &str) -> String {
    let trimmed = endpoint.trim_matches('"').trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{}", trimmed)
    }
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(default)
}
