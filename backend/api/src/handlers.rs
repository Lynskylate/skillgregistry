use crate::models::ApiResponse;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use common::entities::{prelude::*, *};
use sea_orm::*;
use serde::{Deserialize, Serialize};
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

#[derive(Serialize)]
pub struct SkillDto {
    pub id: i32,
    pub name: String,
    pub owner: String,
    pub repo: String,
    pub latest_version: Option<String>,
    pub description: Option<String>,
    pub created_at: chrono::NaiveDateTime,
}

pub async fn list_skills(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Json<ApiResponse<Vec<SkillDto>>> {
    let db = &state.db;
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);

    // Start with finding skills and also related registry
    let mut query = Skills::find().find_also_related(SkillRegistry);

    // Filter by Owner
    if let Some(owner) = &params.owner {
        query = query.filter(skill_registry::Column::Owner.eq(owner));
    }

    // Filter by Repo (Registry Name)
    if let Some(repo) = &params.repo {
        query = query.filter(skill_registry::Column::Name.eq(repo));
    }

    // Filter by Search Query (Skill Name)
    if let Some(q) = &params.q {
        query = query.filter(skills::Column::Name.contains(q));
    }

    // Sort logic
    match params.sort_by.as_deref() {
        Some("name") => {
            query = if params.order.as_deref() == Some("desc") {
                query.order_by_desc(skills::Column::Name)
            } else {
                query.order_by_asc(skills::Column::Name)
            };
        }
        _ => {
            // Default sort by created_at desc
            query = query.order_by_desc(skills::Column::CreatedAt);
        }
    }

    // Pagination
    let paginator = query.paginate(db, per_page);
    let skills_res = paginator.fetch_page(page - 1).await;

    match skills_res {
        Ok(items) => {
            let mut dtos = Vec::new();
            for (skill, registry_opt) in items {
                if let Some(registry) = registry_opt {
                    // Fetch latest version description
                    // Optimization: This is still N+1 but acceptable for small page size (20)
                    let latest_desc = if let Some(v) = &skill.latest_version {
                        SkillVersions::find()
                            .filter(skill_versions::Column::SkillId.eq(skill.id))
                            .filter(skill_versions::Column::Version.eq(v))
                            .one(db)
                            .await
                            .ok()
                            .flatten()
                            .and_then(|sv| sv.description)
                    } else {
                        None
                    };

                    dtos.push(SkillDto {
                        id: skill.id,
                        name: skill.name,
                        owner: registry.owner,
                        repo: registry.name,
                        latest_version: skill.latest_version,
                        description: latest_desc,
                        created_at: skill.created_at,
                    });
                }
            }
            Json(ApiResponse::success(dtos))
        }
        Err(e) => Json(ApiResponse::error(500, e.to_string())),
    }
}

pub async fn get_skill(
    State(state): State<Arc<AppState>>,
    Path((owner, repo, name)): Path<(String, String, String)>,
) -> Json<ApiResponse<serde_json::Value>> {
    let db = &state.db;

    // Find Registry first
    let registry = match SkillRegistry::find()
        .filter(skill_registry::Column::Owner.eq(&owner))
        .filter(skill_registry::Column::Name.eq(&repo))
        .one(db)
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return Json(ApiResponse::error(404, "Repository not found".to_string())),
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    // Find Skill
    let skill = match Skills::find()
        .filter(skills::Column::SkillRegistryId.eq(registry.id))
        .filter(skills::Column::Name.eq(&name))
        .one(db)
        .await
    {
        Ok(Some(s)) => s,
        Ok(None) => return Json(ApiResponse::error(404, "Skill not found".to_string())),
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    let versions = match skill.find_related(SkillVersions).all(db).await {
        Ok(v) => v,
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    Json(ApiResponse::success(serde_json::json!({
        "skill": skill,
        "versions": versions,
        "registry": registry
    })))
}

pub async fn get_skill_version(
    State(state): State<Arc<AppState>>,
    Path((owner, repo, name, version)): Path<(String, String, String, String)>,
) -> Json<ApiResponse<serde_json::Value>> {
    let db = &state.db;

    // Find Registry
    let registry = match SkillRegistry::find()
        .filter(skill_registry::Column::Owner.eq(&owner))
        .filter(skill_registry::Column::Name.eq(&repo))
        .one(db)
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return Json(ApiResponse::error(404, "Repository not found".to_string())),
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    // Find Skill
    let skill = match Skills::find()
        .filter(skills::Column::SkillRegistryId.eq(registry.id))
        .filter(skills::Column::Name.eq(&name))
        .one(db)
        .await
    {
        Ok(Some(s)) => s,
        Ok(None) => return Json(ApiResponse::error(404, "Skill not found".to_string())),
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    // Find Version
    let skill_version = match SkillVersions::find()
        .filter(skill_versions::Column::SkillId.eq(skill.id))
        .filter(skill_versions::Column::Version.eq(&version))
        .one(db)
        .await
    {
        Ok(Some(v)) => v,
        Ok(None) => return Json(ApiResponse::error(404, "Version not found".to_string())),
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    Json(ApiResponse::success(serde_json::json!(skill_version)))
}
