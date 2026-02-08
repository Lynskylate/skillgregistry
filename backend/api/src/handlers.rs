use crate::models::ApiResponse;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::Utc;
use common::entities::discovery_registries;
use common::plugins::{PluginListItemDto, SkillSummaryDto};
use common::repositories::skills::ListSkillsParams;
use common::skills::{DownloadSkillResult, PaginatedSkillsResponse};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use temporalio_client::{ClientOptions, WorkflowClientTrait, WorkflowOptions};
use temporalio_sdk_core::Url as TemporalUrl;
use url::Url;

const DEFAULT_GITHUB_API_URL: &str = "https://api.github.com";

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub host: Option<String>,
    pub org: Option<String>,
    pub sort_by: Option<String>,
    pub order: Option<String>,
    pub compatibility: Option<String>,
    pub has_version: Option<bool>,
}

#[derive(Serialize)]
pub struct DiscoveryRegistryDto {
    pub id: i32,
    pub provider: String,
    pub url: String,
    pub queries: Vec<String>,
    pub schedule_interval_seconds: i64,
    pub token_configured: bool,
    pub last_health_status: Option<String>,
    pub last_health_message: Option<String>,
    pub last_health_checked_at: Option<chrono::NaiveDateTime>,
    pub last_run_at: Option<chrono::NaiveDateTime>,
    pub last_run_status: Option<String>,
    pub last_run_message: Option<String>,
    pub next_run_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Deserialize)]
pub struct CreateDiscoveryRegistryRequest {
    pub provider: String,
    pub token: String,
    pub url: Option<String>,
    pub queries: Vec<String>,
    pub schedule_interval_seconds: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateDiscoveryRegistryRequest {
    pub queries: Vec<String>,
    pub schedule_interval_seconds: i64,
    pub url: String,
}

#[derive(Serialize)]
pub struct DiscoveryRegistryHealthTestDto {
    pub ok: bool,
    pub message: String,
    pub checked_at: chrono::NaiveDateTime,
    pub started_at: Option<chrono::NaiveDateTime>,
}

#[derive(Serialize)]
pub struct TriggerWorkflowDto {
    pub ok: bool,
    pub message: String,
    pub workflow_id: String,
    pub started_at: chrono::NaiveDateTime,
}

#[derive(Serialize)]
pub struct DownloadSkillResponse {
    pub download_url: String,
    pub expires_at: String,
    pub md5: Option<String>,
    pub version: String,
    pub file_size: Option<i64>,
}

#[derive(Serialize)]
pub struct ValidateDeleteResponse {
    pub can_delete: bool,
    pub reasons: Vec<String>,
}

#[derive(Deserialize)]
pub struct DeleteDiscoveryRegistryRequest {
    pub confirmation_id: Option<String>,
}

fn map_provider(platform: &discovery_registries::Platform) -> String {
    match platform {
        discovery_registries::Platform::Github => "github".to_string(),
    }
}

fn normalize_api_url(raw: Option<&str>) -> Result<String, String> {
    let value = raw
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(DEFAULT_GITHUB_API_URL);

    let parsed = Url::parse(value).map_err(|e| format!("invalid url: {}", e))?;

    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("url must use http or https".to_string());
    }

    if parsed.host_str().is_none() {
        return Err("url must include a host".to_string());
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("url must not include credentials".to_string());
    }

    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("url must not include query or fragment".to_string());
    }

    Ok(parsed.as_str().trim_end_matches('/').to_string())
}

fn to_registry_dto(
    config: &common::services::discovery_registries::DiscoveryRegistryConfig,
) -> DiscoveryRegistryDto {
    DiscoveryRegistryDto {
        id: config.id,
        provider: map_provider(&config.platform),
        url: config.api_url.clone(),
        queries: config.queries.clone(),
        schedule_interval_seconds: config.schedule_interval_seconds,
        token_configured: !config.token.trim().is_empty(),
        last_health_status: config.last_health_status.clone(),
        last_health_message: config.last_health_message.clone(),
        last_health_checked_at: config.last_health_checked_at,
        last_run_at: config.last_run_at,
        last_run_status: config.last_run_status.clone(),
        last_run_message: config.last_run_message.clone(),
        next_run_at: config.next_run_at,
        created_at: config.created_at,
        updated_at: config.updated_at,
    }
}

fn normalize_queries(input: Vec<String>) -> Vec<String> {
    input
        .into_iter()
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty())
        .collect()
}

fn is_admin(user: &crate::auth::AuthUser) -> bool {
    user.role == "admin"
}

fn create_json_payload(
    data: &impl serde::Serialize,
) -> temporalio_common::protos::temporal::api::common::v1::Payload {
    temporalio_common::protos::temporal::api::common::v1::Payload {
        metadata: std::collections::HashMap::from([(
            "encoding".to_string(),
            "json/plain".as_bytes().to_vec(),
        )]),
        data: serde_json::to_vec(data).expect("failed to serialize trigger payload"),
        ..Default::default()
    }
}

pub async fn list_skills(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Json<ApiResponse<PaginatedSkillsResponse>> {
    let list_params = ListSkillsParams {
        host: params.host.as_deref(),
        org: params.org.as_deref(),
        owner: params.owner.as_deref(),
        repo: params.repo.as_deref(),
        query: params.q.as_deref(),
        sort_by: params.sort_by.as_deref(),
        order: params.order.as_deref(),
        compatibility: params.compatibility.as_deref(),
        has_version: params.has_version,
        page: params.page.unwrap_or(1),
        per_page: params.per_page.unwrap_or(20),
    };

    match state.services.skill_service.list_skills(list_params).await {
        Ok(result) => Json(ApiResponse::success(result)),
        Err(e) => Json(ApiResponse::error(e.code, e.message)),
    }
}

pub async fn get_repo_skill_detail(
    State(state): State<Arc<AppState>>,
    Path((host, org, repo, name)): Path<(String, String, String, String)>,
) -> Json<ApiResponse<common::skills::SkillDetail>> {
    match state
        .services
        .skill_service
        .get_skill_by_host(&host, &org, &repo, &name)
        .await
    {
        Ok(result) => Json(ApiResponse::success(result)),
        Err(e) => Json(ApiResponse::error(e.code, e.message)),
    }
}

pub async fn get_repo_skill_version(
    State(state): State<Arc<AppState>>,
    Path((host, org, repo, name, version)): Path<(String, String, String, String, String)>,
) -> Json<ApiResponse<common::skills::SkillVersionDetail>> {
    match state
        .services
        .skill_service
        .get_skill_version_by_host(&host, &org, &repo, &name, &version)
        .await
    {
        Ok(result) => Json(ApiResponse::success(result)),
        Err(e) => Json(ApiResponse::error(e.code, e.message)),
    }
}

fn to_download_response(result: DownloadSkillResult) -> DownloadSkillResponse {
    DownloadSkillResponse {
        download_url: result.download_url,
        expires_at: result.expires_at.to_rfc3339(),
        md5: result.md5,
        version: result.version,
        file_size: result.file_size,
    }
}

pub async fn download_repo_skill(
    State(state): State<Arc<AppState>>,
    Path((host, org, repo, name)): Path<(String, String, String, String)>,
) -> Json<ApiResponse<DownloadSkillResponse>> {
    match state
        .services
        .skill_service
        .download_skill(&host, &org, &repo, &name)
        .await
    {
        Ok(result) => Json(ApiResponse::success(to_download_response(result))),
        Err(e) => Json(ApiResponse::error(e.code, e.message)),
    }
}

pub async fn list_repo_plugins(
    State(state): State<Arc<AppState>>,
    Path((host, org, repo)): Path<(String, String, String)>,
) -> Json<ApiResponse<Vec<PluginListItemDto>>> {
    match state
        .services
        .plugin_service
        .list_repo_plugins(&host, &org, &repo)
        .await
    {
        Ok(dtos) => Json(ApiResponse::success(dtos)),
        Err(e) => Json(ApiResponse::error(e.code, e.message)),
    }
}

pub async fn get_repo_plugin(
    State(state): State<Arc<AppState>>,
    Path((host, org, repo, plugin_name)): Path<(String, String, String, String)>,
) -> Json<ApiResponse<serde_json::Value>> {
    match state
        .services
        .plugin_service
        .get_repo_plugin(&host, &org, &repo, &plugin_name)
        .await
    {
        Ok(result) => match serde_json::to_value(&result) {
            Ok(value) => Json(ApiResponse::success(value)),
            Err(e) => Json(ApiResponse::error(
                500,
                format!("Serialization error: {}", e),
            )),
        },
        Err(e) => Json(ApiResponse::error(e.code, e.message)),
    }
}

pub async fn list_repo_skills(
    State(state): State<Arc<AppState>>,
    Path((host, org, repo)): Path<(String, String, String)>,
) -> Json<ApiResponse<Vec<SkillSummaryDto>>> {
    match state
        .services
        .plugin_service
        .list_repo_skills(&host, &org, &repo)
        .await
    {
        Ok(dtos) => Json(ApiResponse::success(dtos)),
        Err(e) => Json(ApiResponse::error(e.code, e.message)),
    }
}

pub async fn get_repo_plugin_agent(
    State(state): State<Arc<AppState>>,
    Path((host, org, repo, plugin_name, agent_name)): Path<(
        String,
        String,
        String,
        String,
        String,
    )>,
) -> Json<ApiResponse<serde_json::Value>> {
    match state
        .services
        .plugin_service
        .get_repo_plugin_component(&host, &org, &repo, &plugin_name, "agent", &agent_name)
        .await
    {
        Ok(v) => match serde_json::to_value(&v) {
            Ok(value) => Json(ApiResponse::success(value)),
            Err(e) => Json(ApiResponse::error(
                500,
                format!("Serialization error: {}", e),
            )),
        },
        Err(e) => Json(ApiResponse::error(e.code, e.message)),
    }
}

pub async fn get_repo_plugin_skill(
    State(state): State<Arc<AppState>>,
    Path((host, org, repo, plugin_name, skill_name)): Path<(
        String,
        String,
        String,
        String,
        String,
    )>,
) -> Json<ApiResponse<serde_json::Value>> {
    match state
        .services
        .plugin_service
        .get_repo_plugin_component(&host, &org, &repo, &plugin_name, "skill", &skill_name)
        .await
    {
        Ok(v) => match serde_json::to_value(&v) {
            Ok(value) => Json(ApiResponse::success(value)),
            Err(e) => Json(ApiResponse::error(
                500,
                format!("Serialization error: {}", e),
            )),
        },
        Err(e) => Json(ApiResponse::error(e.code, e.message)),
    }
}

pub async fn get_repo_plugin_command(
    State(state): State<Arc<AppState>>,
    Path((host, org, repo, plugin_name, command_name)): Path<(
        String,
        String,
        String,
        String,
        String,
    )>,
) -> Json<ApiResponse<serde_json::Value>> {
    match state
        .services
        .plugin_service
        .get_repo_plugin_component(&host, &org, &repo, &plugin_name, "command", &command_name)
        .await
    {
        Ok(v) => match serde_json::to_value(&v) {
            Ok(value) => Json(ApiResponse::success(value)),
            Err(e) => Json(ApiResponse::error(
                500,
                format!("Serialization error: {}", e),
            )),
        },
        Err(e) => Json(ApiResponse::error(e.code, e.message)),
    }
}

pub async fn list_discovery_registries(
    State(state): State<Arc<AppState>>,
    user: crate::auth::AuthUser,
) -> Json<ApiResponse<Vec<DiscoveryRegistryDto>>> {
    if !is_admin(&user) {
        return Json(ApiResponse::error(403, "admin access required".to_string()));
    }

    match state.services.discovery_registry_service.list_all().await {
        Ok(configs) => {
            let rows = configs.iter().map(to_registry_dto).collect();
            Json(ApiResponse::success(rows))
        }
        Err(e) => Json(ApiResponse::error(500, e.to_string())),
    }
}

pub async fn create_discovery_registry(
    State(state): State<Arc<AppState>>,
    user: crate::auth::AuthUser,
    Json(req): Json<CreateDiscoveryRegistryRequest>,
) -> Json<ApiResponse<DiscoveryRegistryDto>> {
    if !is_admin(&user) {
        return Json(ApiResponse::error(403, "admin access required".to_string()));
    }

    if req.provider.trim().to_lowercase() != "github" {
        return Json(ApiResponse::error(
            400,
            "only github provider is supported".to_string(),
        ));
    }

    let token = req.token.trim().to_string();
    if token.is_empty() {
        return Json(ApiResponse::error(400, "token is required".to_string()));
    }

    let queries = normalize_queries(req.queries);
    if queries.is_empty() {
        return Json(ApiResponse::error(
            400,
            "queries cannot be empty".to_string(),
        ));
    }

    if req.schedule_interval_seconds < 60 {
        return Json(ApiResponse::error(
            400,
            "schedule_interval_seconds must be at least 60".to_string(),
        ));
    }

    let api_url = match normalize_api_url(req.url.as_deref()) {
        Ok(url) => url,
        Err(msg) => return Json(ApiResponse::error(400, msg)),
    };

    match state
        .services
        .discovery_registry_service
        .create_github_registry(token, queries, req.schedule_interval_seconds, api_url)
        .await
    {
        Ok(config) => Json(ApiResponse::success(to_registry_dto(&config))),
        Err(e) => Json(ApiResponse::error(500, e.to_string())),
    }
}

pub async fn update_discovery_registry(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    user: crate::auth::AuthUser,
    Json(req): Json<UpdateDiscoveryRegistryRequest>,
) -> Json<ApiResponse<DiscoveryRegistryDto>> {
    if !is_admin(&user) {
        return Json(ApiResponse::error(403, "admin access required".to_string()));
    }

    let queries = normalize_queries(req.queries);
    if queries.is_empty() {
        return Json(ApiResponse::error(
            400,
            "queries cannot be empty".to_string(),
        ));
    }

    if req.schedule_interval_seconds < 60 {
        return Json(ApiResponse::error(
            400,
            "schedule_interval_seconds must be at least 60".to_string(),
        ));
    }

    let api_url = match normalize_api_url(Some(&req.url)) {
        Ok(url) => url,
        Err(msg) => return Json(ApiResponse::error(400, msg)),
    };

    match state
        .services
        .discovery_registry_service
        .update_config(id, queries, req.schedule_interval_seconds, api_url)
        .await
    {
        Ok(Some(config)) => Json(ApiResponse::success(to_registry_dto(&config))),
        Ok(None) => Json(ApiResponse::error(404, "registry not found".to_string())),
        Err(e) => Json(ApiResponse::error(500, e.to_string())),
    }
}

pub async fn validate_delete_discovery_registry(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    user: crate::auth::AuthUser,
) -> Json<ApiResponse<ValidateDeleteResponse>> {
    if !is_admin(&user) {
        return Json(ApiResponse::error(403, "admin access required".to_string()));
    }

    let config = match state
        .services
        .discovery_registry_service
        .find_by_id(id)
        .await
    {
        Ok(Some(cfg)) => cfg,
        Ok(None) => return Json(ApiResponse::error(404, "registry not found".to_string())),
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    let mut reasons =
        vec!["Deleting this registry prevents future discovery runs from this source.".to_string()];
    if config.next_run_at.is_some() {
        reasons.push("A future discovery run is currently scheduled.".to_string());
    }

    Json(ApiResponse::success(ValidateDeleteResponse {
        can_delete: true,
        reasons,
    }))
}

pub async fn delete_discovery_registry(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    user: crate::auth::AuthUser,
    payload: Option<Json<DeleteDiscoveryRegistryRequest>>,
) -> Json<ApiResponse<serde_json::Value>> {
    if !is_admin(&user) {
        return Json(ApiResponse::error(403, "admin access required".to_string()));
    }

    let confirmation_id = payload.and_then(|Json(req)| req.confirmation_id);
    if let Some(conf) = confirmation_id {
        if conf != id.to_string() {
            return Json(ApiResponse::error(
                400,
                "Confirmation ID does not match registry ID".to_string(),
            ));
        }
    }

    match state
        .services
        .discovery_registry_service
        .delete_by_id(id)
        .await
    {
        Ok(true) => Json(ApiResponse::success(serde_json::json!({"deleted": true}))),
        Ok(false) => Json(ApiResponse::error(404, "registry not found".to_string())),
        Err(e) => Json(ApiResponse::error(500, e.to_string())),
    }
}

pub async fn test_discovery_registry_health(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    user: crate::auth::AuthUser,
) -> Json<ApiResponse<DiscoveryRegistryHealthTestDto>> {
    if !is_admin(&user) {
        return Json(ApiResponse::error(403, "admin access required".to_string()));
    }

    let config = match state
        .services
        .discovery_registry_service
        .find_by_id(id)
        .await
    {
        Ok(Some(cfg)) => cfg,
        Ok(None) => return Json(ApiResponse::error(404, "registry not found".to_string())),
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    let started_at = Utc::now().naive_utc();
    let checked_at = Utc::now().naive_utc();
    let client = reqwest::Client::new();
    let health_url = format!("{}/rate_limit", config.api_url.trim_end_matches('/'));

    let response = client
        .get(health_url)
        .header("User-Agent", "skillregistry")
        .header("Accept", "application/vnd.github+json")
        .bearer_auth(config.token)
        .send()
        .await;

    let (ok, message) = match response {
        Ok(res) if res.status().is_success() => (true, "GitHub API reachable".to_string()),
        Ok(res) => (
            false,
            format!("GitHub API returned status {}", res.status().as_u16()),
        ),
        Err(e) => (false, format!("GitHub API request failed: {}", e)),
    };

    let status = if ok { "ok" } else { "error" }.to_string();
    if let Err(e) = state
        .services
        .discovery_registry_service
        .update_health(id, status, Some(message.clone()), checked_at)
        .await
    {
        return Json(ApiResponse::error(500, e.to_string()));
    }

    Json(ApiResponse::success(DiscoveryRegistryHealthTestDto {
        ok,
        message,
        checked_at,
        started_at: Some(started_at),
    }))
}

pub async fn trigger_discovery_registry(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    user: crate::auth::AuthUser,
) -> Json<ApiResponse<TriggerWorkflowDto>> {
    if !is_admin(&user) {
        return Json(ApiResponse::error(403, "admin access required".to_string()));
    }

    let exists = match state
        .services
        .discovery_registry_service
        .find_by_id(id)
        .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    if !exists {
        return Json(ApiResponse::error(404, "registry not found".to_string()));
    }

    let started_at = Utc::now().naive_utc();
    let workflow_id = format!("trigger-registry-{}-{}", id, uuid::Uuid::new_v4());
    let task_queue = state.settings.temporal.task_queue.clone();

    let temporal_url = match TemporalUrl::from_str(&state.settings.temporal.server_url) {
        Ok(url) => url,
        Err(e) => {
            return Json(ApiResponse::error(
                500,
                format!("invalid temporal server url: {}", e),
            ))
        }
    };

    let client_options = ClientOptions::builder()
        .target_url(temporal_url)
        .client_name("skillregistry-api")
        .client_version("0.1.0")
        .build();

    let client = match client_options.connect("default", None).await {
        Ok(client) => client,
        Err(e) => {
            return Json(ApiResponse::error(
                500,
                format!("failed to connect to temporal: {}", e),
            ))
        }
    };

    let payload = create_json_payload(&id);

    let opts = WorkflowOptions::default();
    if let Err(e) = client
        .start_workflow(
            vec![payload],
            task_queue,
            workflow_id.clone(),
            "trigger_registry_workflow".to_string(),
            None,
            opts,
        )
        .await
    {
        return Json(ApiResponse::error(
            500,
            format!("failed to trigger workflow: {}", e),
        ));
    }

    Json(ApiResponse::success(TriggerWorkflowDto {
        ok: true,
        message: "Discovery workflow triggered".to_string(),
        workflow_id,
        started_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::normalize_api_url;

    #[test]
    fn normalize_api_url_defaults_to_public_github() {
        let normalized = normalize_api_url(None).expect("default URL should be valid");
        assert_eq!(normalized, "https://api.github.com");
    }

    #[test]
    fn normalize_api_url_accepts_enterprise_api_base() {
        let normalized = normalize_api_url(Some("https://ghe.example.com/api/v3/"))
            .expect("enterprise URL should be valid");
        assert_eq!(normalized, "https://ghe.example.com/api/v3");
    }

    #[test]
    fn normalize_api_url_rejects_query_string() {
        let err = normalize_api_url(Some("https://api.github.com/?a=1"))
            .expect_err("query parameters are not allowed");
        assert!(err.contains("query"));
    }
}
