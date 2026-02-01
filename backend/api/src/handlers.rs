use axum::{
    extract::{Path, Query, State},
    Json,
};
use common::entities::{prelude::*, *};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::AppState;

#[derive(Deserialize)]
pub struct SearchParams {
    q: Option<String>,
    page: Option<u64>,
    per_page: Option<u64>,
}

#[derive(Serialize)]
pub struct SkillDto {
    pub name: String,
    pub latest_version: Option<String>,
    pub description: Option<String>,
    pub created_at: chrono::NaiveDateTime,
}

pub async fn list_skills(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<SkillDto>>, String> {
    let db = &state.db;
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);

    let mut query = Skills::find();

    if let Some(q) = params.q {
        query = query.filter(skills::Column::Name.contains(&q));
    }
    
    let skills = query
        .paginate(db, per_page)
        .fetch_page(page - 1)
        .await
        .map_err(|e| e.to_string())?;

    let mut dtos = Vec::new();
    for skill in skills {
        // Fetch latest version description if needed
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
            name: skill.name,
            latest_version: skill.latest_version,
            description: latest_desc,
            created_at: skill.created_at,
        });
    }

    Ok(Json(dtos))
}

pub async fn get_skill(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, String> {
    let db = &state.db;
    
    let skill = Skills::find()
        .filter(skills::Column::Name.eq(&name))
        .one(db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Skill not found".to_string())?;

    let versions = skill.find_related(SkillVersions).all(db).await.map_err(|e| e.to_string())?;

    Ok(Json(serde_json::json!({
        "skill": skill,
        "versions": versions
    })))
}

pub async fn get_skill_version(
    State(state): State<Arc<AppState>>,
    Path((name, version)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, String> {
    let db = &state.db;

    let skill = Skills::find()
        .filter(skills::Column::Name.eq(&name))
        .one(db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Skill not found".to_string())?;

    let skill_version = SkillVersions::find()
        .filter(skill_versions::Column::SkillId.eq(skill.id))
        .filter(skill_versions::Column::Version.eq(&version))
        .one(db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Version not found".to_string())?;

    Ok(Json(serde_json::json!(skill_version)))
}
