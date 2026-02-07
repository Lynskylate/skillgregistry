use anyhow::{anyhow, bail, Context, Result};
use aws_config::BehaviorVersion;
use aws_credential_types::{provider::SharedCredentialsProvider, Credentials};
use aws_sdk_s3::Client as S3Client;
use chrono::{NaiveDateTime, Utc};
use common::entities::{
    plugin_versions, plugins,
    prelude::{DiscoveryRegistries, PluginVersions, Plugins, SkillRegistry, SkillVersions, Skills},
    skill_registry, skill_versions, skills,
};
use reqwest::StatusCode;
use sea_orm::{
    ColumnTrait, Database, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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
    api_base_url: String,
    admin_username: String,
    admin_password: String,
    discovery_query: String,
    target_owner: String,
    target_repo: String,
    query_recent_skill_raw: String,
    query_marketplace_raw: String,
    query_poll_timeout: Duration,
    baseline_discovery_registry_count: usize,
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
                "SKILLREGISTRY_DATABASE__URL",
                "sqlite:///tmp/skillregistry-e2e.db?mode=rwc",
            ),
            temporal_server_url: env_or(
                "SKILLREGISTRY_TEMPORAL__SERVER_URL",
                "http://localhost:7233",
            ),
            temporal_task_queue: env_or(
                "SKILLREGISTRY_TEMPORAL__TASK_QUEUE",
                "skill-registry-queue",
            ),
            api_base_url: env_or("E2E_API_BASE_URL", "http://localhost:3000"),
            admin_username: env_or("E2E_ADMIN_USERNAME", "admin"),
            admin_password: env_or("E2E_ADMIN_PASSWORD", "admin"),
            discovery_query,
            target_owner,
            target_repo,
            query_recent_skill_raw: env_or(
                "E2E_QUERY_RECENT_SKILL_RAW",
                "path:**/SKILL.md --- \"name:\" \"description:\" NOT is:fork",
            ),
            query_marketplace_raw: env_or(
                "E2E_QUERY_MARKETPLACE_RAW",
                "path:.claude-plugin/marketplace.json NOT is:fork",
            ),
            query_poll_timeout: Duration::from_secs(env_u64("E2E_QUERY_POLL_TIMEOUT_SECS", 900)),
            baseline_discovery_registry_count: env_u64("E2E_BASELINE_DISCOVERY_REGISTRY_COUNT", 1)
                as usize,
            discovery_timeout: Duration::from_secs(env_u64("E2E_DISCOVERY_TIMEOUT_SECS", 240)),
            sync_timeout: Duration::from_secs(env_u64("E2E_SYNC_TIMEOUT_SECS", 900)),
            s3_bucket: env_or("SKILLREGISTRY_S3__BUCKET", "skills"),
            s3_region: env_or("SKILLREGISTRY_S3__REGION", "us-east-1"),
            s3_endpoint: std::env::var("SKILLREGISTRY_S3__ENDPOINT")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            s3_force_path_style: env_bool("SKILLREGISTRY_S3__FORCE_PATH_STYLE", true),
            aws_access_key_id: std::env::var("SKILLREGISTRY_S3__ACCESS_KEY_ID")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            aws_secret_access_key: std::env::var("SKILLREGISTRY_S3__SECRET_ACCESS_KEY")
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

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    code: i32,
    message: String,
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct DiscoveryRegistryDto {
    id: i32,
    queries: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TriggerWorkflowDto {
    workflow_id: String,
}

#[derive(Debug, Deserialize, Clone)]
struct SkillListItemDto {
    name: String,
    owner: String,
    repo: String,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct SkillKey {
    owner: String,
    repo: String,
    name: String,
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

#[tokio::test]
#[ignore = "requires API, worker, Temporal, RustFS/S3, SQLite, and a valid GITHUB_TOKEN"]
async fn test_admin_registry_trigger_and_index_two_queries() -> Result<()> {
    let cfg = E2eConfig::from_env()?;
    let db = Database::connect(&cfg.database_url)
        .await
        .context("failed to connect to sqlite database")?;

    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .context("failed to build HTTP client")?;

    wait_for_min_discovery_registry_count(
        &db,
        cfg.baseline_discovery_registry_count,
        cfg.query_poll_timeout,
    )
    .await?;

    assert_admin_endpoint_requires_auth(&client, &cfg).await?;

    let non_admin_token = register_local_user_and_get_token(&client, &cfg).await?;
    assert_non_admin_forbidden(&client, &cfg, &non_admin_token).await?;

    let admin_token = login_admin_and_get_token(&client, &cfg).await?;

    let normalized_recent_query = normalize_query_for_api(&cfg.query_recent_skill_raw, true);
    let normalized_marketplace_query = normalize_query_for_api(&cfg.query_marketplace_raw, false);

    println!("Normalized recent query: {}", normalized_recent_query);
    println!(
        "Normalized marketplace query: {}",
        normalized_marketplace_query
    );

    let recent_registry = create_discovery_registry_via_admin_api(
        &client,
        &cfg,
        &admin_token,
        vec![normalized_recent_query.clone()],
    )
    .await?;

    let expected_recent_query = vec![normalize_query_for_api(&normalized_recent_query, false)];
    if recent_registry.queries != expected_recent_query {
        bail!(
            "recent registry stored unexpected query list: {:?}",
            recent_registry.queries
        );
    }

    let recent_triggered_at = Utc::now().naive_utc();
    let recent_trigger =
        trigger_discovery_registry_via_admin_api(&client, &cfg, &admin_token, recent_registry.id)
            .await?;
    println!(
        "Triggered recent query registry id={} workflow_id={}",
        recent_registry.id, recent_trigger.workflow_id
    );

    wait_for_registry_run(
        &db,
        recent_registry.id,
        recent_triggered_at,
        cfg.query_poll_timeout,
    )
    .await?;
    wait_for_registry_repo_count(&db, recent_registry.id, 1, cfg.query_poll_timeout).await?;

    let marketplace_registry = create_discovery_registry_via_admin_api(
        &client,
        &cfg,
        &admin_token,
        vec![normalized_marketplace_query.clone()],
    )
    .await?;

    let expected_marketplace_query = vec![normalize_query_for_api(
        &normalized_marketplace_query,
        false,
    )];
    if marketplace_registry.queries != expected_marketplace_query {
        bail!(
            "marketplace registry stored unexpected query list: {:?}",
            marketplace_registry.queries
        );
    }

    let marketplace_triggered_at = Utc::now().naive_utc();
    let marketplace_trigger = trigger_discovery_registry_via_admin_api(
        &client,
        &cfg,
        &admin_token,
        marketplace_registry.id,
    )
    .await?;
    println!(
        "Triggered marketplace query registry id={} workflow_id={}",
        marketplace_registry.id, marketplace_trigger.workflow_id
    );

    wait_for_registry_run(
        &db,
        marketplace_registry.id,
        marketplace_triggered_at,
        cfg.query_poll_timeout,
    )
    .await?;
    wait_for_registry_repo_count(&db, marketplace_registry.id, 1, cfg.query_poll_timeout).await?;

    let marketplace_repo_count = count_registry_repos(&db, marketplace_registry.id).await?;
    if marketplace_repo_count < 1 {
        bail!(
            "marketplace query registry id={} indexed no repositories",
            marketplace_registry.id
        );
    }

    let db_skills = wait_for_db_indexed_skills(&db, cfg.query_poll_timeout).await?;
    let api_skills = fetch_all_skills_from_api(&client, &cfg).await?;

    let db_set: HashSet<SkillKey> = db_skills.into_iter().collect();
    let api_set: HashSet<SkillKey> = api_skills.clone().into_iter().collect();

    if db_set != api_set {
        bail!(
            "API /api/skills mismatch with DB indexed skills: db_count={}, api_count={}",
            db_set.len(),
            api_set.len()
        );
    }

    let page_size = 20;
    let first_page_api = fetch_skills_page_from_api(&client, &cfg, 1, page_size).await?;
    if first_page_api.is_empty() {
        bail!("index API first page is empty after trigger workflow");
    }

    for key in &first_page_api {
        if !db_set.contains(key) {
            bail!(
                "index page item not found in DB snapshot: {}/{}/{}",
                key.owner,
                key.repo,
                key.name
            );
        }
    }

    if api_skills.len() > page_size as usize {
        let second_page_api = fetch_skills_page_from_api(&client, &cfg, 2, page_size).await?;
        for key in &second_page_api {
            if !db_set.contains(key) {
                bail!(
                    "index page 2 item not found in DB snapshot: {}/{}/{}",
                    key.owner,
                    key.repo,
                    key.name
                );
            }
        }
    }

    Ok(())
}

async fn assert_admin_endpoint_requires_auth(
    client: &reqwest::Client,
    cfg: &E2eConfig,
) -> Result<()> {
    let response = client
        .get(format!(
            "{}/api/admin/discovery-registries",
            cfg.api_base_url
        ))
        .send()
        .await
        .context("failed to call admin endpoint without auth")?;

    let (status, envelope): (StatusCode, ApiEnvelope<serde_json::Value>) =
        parse_api_envelope(response, "admin list without auth").await?;

    if status != StatusCode::UNAUTHORIZED || envelope.code != 401 {
        bail!(
            "expected unauthenticated admin call to fail with HTTP 401/code 401, got HTTP {} code {} message '{}'",
            status,
            envelope.code,
            envelope.message
        );
    }

    Ok(())
}

async fn register_local_user_and_get_token(
    client: &reqwest::Client,
    cfg: &E2eConfig,
) -> Result<String> {
    let suffix = Uuid::new_v4().simple().to_string();
    let username = format!("e2e-user-{}", &suffix[..12]);
    let password = format!("P@ssw0rd-{}", &suffix[..12]);
    let email = format!("{}@example.com", username);

    let response = client
        .post(format!("{}/api/auth/register", cfg.api_base_url))
        .json(&serde_json::json!({
            "username": username,
            "password": password,
            "email": email,
            "display_name": "E2E User"
        }))
        .send()
        .await
        .context("failed to register local non-admin test user")?;

    let (_status, envelope): (StatusCode, ApiEnvelope<LoginResponse>) =
        parse_api_envelope(response, "register non-admin user").await?;

    let login = require_api_success(envelope, "register non-admin user")?;
    Ok(login.access_token)
}

async fn assert_non_admin_forbidden(
    client: &reqwest::Client,
    cfg: &E2eConfig,
    token: &str,
) -> Result<()> {
    let response = client
        .get(format!(
            "{}/api/admin/discovery-registries",
            cfg.api_base_url
        ))
        .bearer_auth(token)
        .send()
        .await
        .context("failed to call admin endpoint as non-admin")?;

    let (_status, envelope): (StatusCode, ApiEnvelope<serde_json::Value>) =
        parse_api_envelope(response, "admin list as non-admin").await?;

    if envelope.code != 403 {
        bail!(
            "expected non-admin access denial code=403, got code={} message='{}'",
            envelope.code,
            envelope.message
        );
    }

    Ok(())
}

async fn login_admin_and_get_token(client: &reqwest::Client, cfg: &E2eConfig) -> Result<String> {
    let response = client
        .post(format!("{}/api/auth/login", cfg.api_base_url))
        .json(&serde_json::json!({
            "identifier": cfg.admin_username,
            "password": cfg.admin_password
        }))
        .send()
        .await
        .context("failed to login as admin")?;

    let (_status, envelope): (StatusCode, ApiEnvelope<LoginResponse>) =
        parse_api_envelope(response, "admin login").await?;

    let login = require_api_success(envelope, "admin login")?;
    Ok(login.access_token)
}

async fn create_discovery_registry_via_admin_api(
    client: &reqwest::Client,
    cfg: &E2eConfig,
    admin_token: &str,
    queries: Vec<String>,
) -> Result<DiscoveryRegistryDto> {
    let response = client
        .post(format!(
            "{}/api/admin/discovery-registries",
            cfg.api_base_url
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "platform": "github",
            "token": std::env::var("GITHUB_TOKEN").unwrap_or_default(),
            "queries": queries,
            "schedule_interval_seconds": 3600
        }))
        .send()
        .await
        .context("failed to create discovery registry via admin API")?;

    let (_status, envelope): (StatusCode, ApiEnvelope<DiscoveryRegistryDto>) =
        parse_api_envelope(response, "create discovery registry").await?;

    require_api_success(envelope, "create discovery registry")
}

async fn trigger_discovery_registry_via_admin_api(
    client: &reqwest::Client,
    cfg: &E2eConfig,
    admin_token: &str,
    registry_id: i32,
) -> Result<TriggerWorkflowDto> {
    let response = client
        .post(format!(
            "{}/api/admin/discovery-registries/{}/trigger",
            cfg.api_base_url, registry_id
        ))
        .bearer_auth(admin_token)
        .send()
        .await
        .with_context(|| format!("failed to trigger discovery registry id={}", registry_id))?;

    let (_status, envelope): (StatusCode, ApiEnvelope<TriggerWorkflowDto>) =
        parse_api_envelope(response, "trigger discovery registry").await?;

    require_api_success(envelope, "trigger discovery registry")
}

async fn wait_for_registry_run(
    db: &DatabaseConnection,
    registry_id: i32,
    triggered_at: NaiveDateTime,
    timeout: Duration,
) -> Result<()> {
    let started = Instant::now();
    loop {
        let model = DiscoveryRegistries::find_by_id(registry_id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow!("discovery registry id={} not found", registry_id))?;

        if let Some(last_run_at) = model.last_run_at {
            if last_run_at >= triggered_at {
                return Ok(());
            }
        }

        if started.elapsed() > timeout {
            bail!(
                "timed out waiting for discovery registry id={} run completion",
                registry_id
            );
        }

        sleep(Duration::from_secs(3)).await;
    }
}

async fn wait_for_registry_repo_count(
    db: &DatabaseConnection,
    registry_id: i32,
    min_count: usize,
    timeout: Duration,
) -> Result<()> {
    let started = Instant::now();
    loop {
        let count = count_registry_repos(db, registry_id).await?;
        if count >= min_count {
            return Ok(());
        }

        if started.elapsed() > timeout {
            bail!(
                "timed out waiting for registry id={} to index at least {} repos",
                registry_id,
                min_count
            );
        }

        sleep(Duration::from_secs(3)).await;
    }
}

async fn wait_for_min_discovery_registry_count(
    db: &DatabaseConnection,
    min_count: usize,
    timeout: Duration,
) -> Result<()> {
    let started = Instant::now();

    loop {
        let current = DiscoveryRegistries::find().all(db).await?.len();
        if current >= min_count {
            return Ok(());
        }

        if started.elapsed() > timeout {
            bail!(
                "timed out waiting for at least {} discovery registries, current={}",
                min_count,
                current
            );
        }

        sleep(Duration::from_secs(1)).await;
    }
}

async fn count_registry_repos(db: &DatabaseConnection, registry_id: i32) -> Result<usize> {
    let repos = SkillRegistry::find()
        .filter(skill_registry::Column::DiscoveryRegistryId.eq(registry_id))
        .filter(skill_registry::Column::Status.ne("blacklisted"))
        .all(db)
        .await?;

    Ok(repos.len())
}

async fn wait_for_db_indexed_skills(
    db: &DatabaseConnection,
    timeout: Duration,
) -> Result<Vec<SkillKey>> {
    let started = Instant::now();
    loop {
        let keys = fetch_all_db_skill_keys(db).await?;
        if !keys.is_empty() {
            return Ok(keys);
        }

        if started.elapsed() > timeout {
            bail!("timed out waiting for indexed skills to appear in DB");
        }

        sleep(Duration::from_secs(3)).await;
    }
}

async fn fetch_all_db_skill_keys(db: &DatabaseConnection) -> Result<Vec<SkillKey>> {
    let mut page = 1;
    let per_page = 200;
    let mut out = Vec::new();

    loop {
        let items = fetch_db_skill_page(db, page, per_page).await?;
        if items.is_empty() {
            break;
        }
        let len = items.len();
        out.extend(items);
        if len < per_page as usize {
            break;
        }
        page += 1;
    }

    Ok(out)
}

async fn fetch_db_skill_page(
    db: &DatabaseConnection,
    page: u64,
    per_page: u64,
) -> Result<Vec<SkillKey>> {
    let query = Skills::find()
        .filter(skills::Column::IsActive.eq(1))
        .find_also_related(SkillRegistry)
        .filter(skill_registry::Column::Status.ne("blacklisted"))
        .order_by_desc(skills::Column::CreatedAt);

    let paginator = query.paginate(db, per_page);
    let rows = paginator.fetch_page(page.saturating_sub(1)).await?;

    let mut out = Vec::new();
    for (skill, registry_opt) in rows {
        if let Some(registry) = registry_opt {
            out.push(SkillKey {
                owner: registry.owner,
                repo: registry.name,
                name: skill.name,
            });
        }
    }

    Ok(out)
}

async fn fetch_all_skills_from_api(
    client: &reqwest::Client,
    cfg: &E2eConfig,
) -> Result<Vec<SkillKey>> {
    let mut page = 1;
    let per_page = 200;
    let mut out = Vec::new();

    loop {
        let items = fetch_skills_page_from_api(client, cfg, page, per_page).await?;
        if items.is_empty() {
            break;
        }
        let len = items.len();
        out.extend(items);
        if len < per_page as usize {
            break;
        }
        page += 1;
    }

    Ok(out)
}

async fn fetch_skills_page_from_api(
    client: &reqwest::Client,
    cfg: &E2eConfig,
    page: u64,
    per_page: u64,
) -> Result<Vec<SkillKey>> {
    let response = client
        .get(format!("{}/api/skills", cfg.api_base_url))
        .query(&[
            ("page", page.to_string()),
            ("per_page", per_page.to_string()),
        ])
        .send()
        .await
        .with_context(|| {
            format!(
                "failed to fetch /api/skills page={} per_page={}",
                page, per_page
            )
        })?;

    let (_status, envelope): (StatusCode, ApiEnvelope<Vec<SkillListItemDto>>) =
        parse_api_envelope(response, "list skills").await?;

    let items = require_api_success(envelope, "list skills")?;

    Ok(items
        .into_iter()
        .map(|item| SkillKey {
            owner: item.owner,
            repo: item.repo,
            name: item.name,
        })
        .collect())
}

async fn parse_api_envelope<T: DeserializeOwned>(
    response: reqwest::Response,
    context: &str,
) -> Result<(StatusCode, ApiEnvelope<T>)> {
    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("{}: failed to read response body", context))?;

    let envelope = serde_json::from_str::<ApiEnvelope<T>>(&body).with_context(|| {
        format!(
            "{}: response is not a valid ApiResponse payload. body={}",
            context, body
        )
    })?;

    Ok((status, envelope))
}

fn require_api_success<T>(envelope: ApiEnvelope<T>, context: &str) -> Result<T> {
    if envelope.code != 200 {
        bail!(
            "{} failed with api code {} and message '{}'",
            context,
            envelope.code,
            envelope.message
        );
    }

    envelope
        .data
        .ok_or_else(|| anyhow!("{} returned success but no data payload", context))
}

fn normalize_query_for_api(raw: &str, add_recent_sort: bool) -> String {
    let tokens: Vec<&str> = raw.split_whitespace().collect();
    let mut normalized = Vec::new();
    let mut idx = 0;
    let mut saw_code_qualifier = false;

    while idx < tokens.len() {
        let token = tokens[idx];

        if token == "---" {
            idx += 1;
            continue;
        }

        if token.to_ascii_lowercase().starts_with("path:")
            || token.to_ascii_lowercase().starts_with("filename:")
            || token.to_ascii_lowercase().starts_with("extension:")
        {
            saw_code_qualifier = true;
        }

        if let Some(path_glob) = token.strip_prefix("path:**/") {
            saw_code_qualifier = true;
            normalized.push(format!("path:{}", path_glob));
            idx += 1;
            continue;
        }

        if token.eq_ignore_ascii_case("NOT")
            && idx + 1 < tokens.len()
            && tokens[idx + 1].eq_ignore_ascii_case("is:fork")
        {
            if !saw_code_qualifier
                && !normalized
                    .iter()
                    .any(|t: &String| t.eq_ignore_ascii_case("fork:false"))
            {
                normalized.push("fork:false".to_string());
            }
            idx += 2;
            continue;
        }

        if token.eq_ignore_ascii_case("-is:fork") {
            if !saw_code_qualifier
                && !normalized
                    .iter()
                    .any(|t: &String| t.eq_ignore_ascii_case("fork:false"))
            {
                normalized.push("fork:false".to_string());
            }
            idx += 1;
            continue;
        }

        normalized.push(token.to_string());
        idx += 1;
    }

    let is_code_search = saw_code_qualifier
        || normalized.iter().any(|t| {
            let lower = t.to_ascii_lowercase();
            lower.starts_with("path:")
                || lower.starts_with("filename:")
                || lower.starts_with("extension:")
        });

    let has_fork_filter = normalized
        .iter()
        .any(|t| t.eq_ignore_ascii_case("fork:false") || t.eq_ignore_ascii_case("fork:true"));
    if !is_code_search && !has_fork_filter {
        normalized.push("fork:false".to_string());
    }

    if add_recent_sort
        && !is_code_search
        && !normalized
            .iter()
            .any(|t| t.to_ascii_lowercase().starts_with("sort:"))
    {
        normalized.push("sort:updated".to_string());
    }

    normalized.join(" ")
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
    let latest_skill_version = SkillVersions::find()
        .filter(skill_versions::Column::CreatedAt.gte(created_after))
        .filter(skill_versions::Column::S3Key.is_not_null())
        .find_also_related(Skills)
        .filter(skills::Column::SkillRegistryId.eq(registry_id))
        .order_by_desc(skill_versions::Column::CreatedAt)
        .one(db)
        .await?
        .and_then(|(version, skill)| {
            let skill = skill?;
            Some(UploadedArtifact {
                source: "skill",
                name: skill.name,
                version: version.version,
                s3_key: version.s3_key?,
                created_at: version.created_at,
            })
        });

    let latest_plugin_version = PluginVersions::find()
        .filter(plugin_versions::Column::CreatedAt.gte(created_after))
        .filter(plugin_versions::Column::S3Key.is_not_null())
        .find_also_related(Plugins)
        .filter(plugins::Column::SkillRegistryId.eq(registry_id))
        .order_by_desc(plugin_versions::Column::CreatedAt)
        .one(db)
        .await?
        .and_then(|(version, plugin)| {
            let plugin = plugin?;
            Some(UploadedArtifact {
                source: "plugin",
                name: plugin.name,
                version: version.version,
                s3_key: version.s3_key?,
                created_at: version.created_at,
            })
        });

    Ok(match (latest_skill_version, latest_plugin_version) {
        (Some(skill), Some(plugin)) => {
            if skill.created_at >= plugin.created_at {
                Some(skill)
            } else {
                Some(plugin)
            }
        }
        (Some(skill), None) => Some(skill),
        (None, Some(plugin)) => Some(plugin),
        (None, None) => None,
    })
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
        data: serde_json::to_vec(input).expect("failed to serialize JSON payload"),
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
        // Most S3-compatible local endpoints (e.g. RustFS/MinIO) default to plain HTTP.
        format!("http://{}", trimmed)
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
