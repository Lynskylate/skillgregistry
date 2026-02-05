use crate::models::ApiResponse;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use common::plugins::{PluginListItemDto, SkillSummaryDto};
use common::repositories::skills::ListSkillsParams;
use common::skills::SkillDto;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub sort_by: Option<String>,
    pub order: Option<String>,
}

pub async fn list_skills(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Json<ApiResponse<Vec<SkillDto>>> {
    let list_params = ListSkillsParams {
        query: params.q.as_deref(),
        owner: params.owner.as_deref(),
        repo: params.repo.as_deref(),
        sort_by: params.sort_by.as_deref(),
        order: params.order.as_deref(),
        page: params.page.unwrap_or(1),
        per_page: params.per_page.unwrap_or(20),
    };
    match state.services.skill_service.list_skills(list_params).await {
        Ok(dtos) => Json(ApiResponse::success(dtos)),
        Err(e) => Json(ApiResponse::error(500, e.to_string())),
    }
}

pub async fn get_skill(
    State(state): State<Arc<AppState>>,
    Path((owner, repo, name)): Path<(String, String, String)>,
) -> Json<ApiResponse<serde_json::Value>> {
    match state
        .services
        .skill_service
        .get_skill(&owner, &repo, &name)
        .await
    {
        Ok(result) => {
            let value = serde_json::to_value(&result).unwrap_or_default();
            Json(ApiResponse::success(value))
        }
        Err(e) => Json(ApiResponse::error(404, e.to_string())),
    }
}

pub async fn get_skill_version(
    State(state): State<Arc<AppState>>,
    Path((owner, repo, name, version)): Path<(String, String, String, String)>,
) -> Json<ApiResponse<serde_json::Value>> {
    match state
        .services
        .skill_service
        .get_skill_version(&owner, &repo, &name, &version)
        .await
    {
        Ok(result) => {
            let value = serde_json::to_value(&result).unwrap_or_default();
            Json(ApiResponse::success(value))
        }
        Err(e) => Json(ApiResponse::error(404, e.to_string())),
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
        Err(e) => Json(ApiResponse::error(500, e.to_string())),
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
        Ok(result) => {
            let value = serde_json::to_value(&result).unwrap_or_default();
            Json(ApiResponse::success(value))
        }
        Err(e) => Json(ApiResponse::error(404, e.to_string())),
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
        Err(e) => Json(ApiResponse::error(500, e.to_string())),
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
        Ok(v) => {
            let value = serde_json::to_value(&v).unwrap_or_default();
            Json(ApiResponse::success(value))
        }
        Err(e) => Json(ApiResponse::error(404, e.to_string())),
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
        Ok(v) => {
            let value = serde_json::to_value(&v).unwrap_or_default();
            Json(ApiResponse::success(value))
        }
        Err(e) => Json(ApiResponse::error(404, e.to_string())),
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
        Ok(v) => {
            let value = serde_json::to_value(&v).unwrap_or_default();
            Json(ApiResponse::success(value))
        }
        Err(e) => Json(ApiResponse::error(404, e.to_string())),
    }
}
