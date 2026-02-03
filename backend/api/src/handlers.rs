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
    let mut query = Skills::find()
        .filter(skills::Column::IsActive.eq(1))
        .find_also_related(SkillRegistry)
        .filter(skill_registry::Column::Status.ne("blacklisted"));

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
        .filter(skill_registry::Column::Status.ne("blacklisted"))
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
        .filter(skills::Column::IsActive.eq(1))
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
        .filter(skill_registry::Column::Status.ne("blacklisted"))
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
        .filter(skills::Column::IsActive.eq(1))
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

#[derive(Serialize)]
pub struct PluginListItemDto {
    pub name: String,
    pub description: Option<String>,
    pub latest_version: Option<String>,
    pub strict: bool,
    pub source: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct SkillSummaryDto {
    pub name: String,
    pub description: Option<String>,
    pub origin: SkillOriginDto,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkillOriginDto {
    Standalone {
        latest_version: Option<String>,
        ref_api: String,
    },
    Plugin {
        plugin_name: String,
        plugin_version: Option<String>,
        ref_api: String,
    },
}

async fn find_registry_by_host(
    db: &DatabaseConnection,
    host: &str,
    org: &str,
    repo: &str,
) -> Result<Option<skill_registry::Model>, DbErr> {
    // Construct exact URLs based on the known format from discovery
    // URLs are stored as https://host/owner/repo or http://host/owner/repo
    let url_https = format!("https://{}/{}/{}", host, org, repo);
    let url_http = format!("http://{}/{}/{}", host, org, repo);

    SkillRegistry::find()
        .filter(skill_registry::Column::Owner.eq(org))
        .filter(skill_registry::Column::Name.eq(repo))
        .filter(skill_registry::Column::Status.ne("blacklisted"))
        .filter(
            Condition::any()
                .add(skill_registry::Column::Url.eq(url_https))
                .add(skill_registry::Column::Url.eq(url_http)),
        )
        .one(db)
        .await
}

pub async fn list_repo_plugins(
    State(state): State<Arc<AppState>>,
    Path((host, org, repo)): Path<(String, String, String)>,
) -> Json<ApiResponse<Vec<PluginListItemDto>>> {
    let db = &state.db;
    let registry = match find_registry_by_host(db, &host, &org, &repo).await {
        Ok(Some(r)) => r,
        Ok(None) => return Json(ApiResponse::error(404, "Repository not found".to_string())),
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    let plugins = match Plugins::find()
        .filter(plugins::Column::SkillRegistryId.eq(registry.id))
        .filter(plugins::Column::IsActive.eq(1))
        .order_by_asc(plugins::Column::Name)
        .all(db)
        .await
    {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    let dtos = plugins
        .into_iter()
        .map(|p| PluginListItemDto {
            name: p.name,
            description: p.description,
            latest_version: p.latest_version,
            strict: p.strict != 0,
            source: p.source,
        })
        .collect();

    Json(ApiResponse::success(dtos))
}

pub async fn get_repo_plugin(
    State(state): State<Arc<AppState>>,
    Path((host, org, repo, plugin_name)): Path<(String, String, String, String)>,
) -> Json<ApiResponse<serde_json::Value>> {
    let db = &state.db;
    let registry = match find_registry_by_host(db, &host, &org, &repo).await {
        Ok(Some(r)) => r,
        Ok(None) => return Json(ApiResponse::error(404, "Repository not found".to_string())),
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    let plugin = match Plugins::find()
        .filter(plugins::Column::SkillRegistryId.eq(registry.id))
        .filter(plugins::Column::Name.eq(&plugin_name))
        .filter(plugins::Column::IsActive.eq(1))
        .one(db)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return Json(ApiResponse::error(404, "Plugin not found".to_string())),
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    let Some(latest_version) = plugin.latest_version.clone() else {
        return Json(ApiResponse::error(
            404,
            "Plugin version not found".to_string(),
        ));
    };

    let plugin_version = match PluginVersions::find()
        .filter(plugin_versions::Column::PluginId.eq(plugin.id))
        .filter(plugin_versions::Column::Version.eq(&latest_version))
        .one(db)
        .await
    {
        Ok(Some(v)) => v,
        Ok(None) => {
            return Json(ApiResponse::error(
                404,
                "Plugin version not found".to_string(),
            ))
        }
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    let components = match PluginComponents::find()
        .filter(plugin_components::Column::PluginVersionId.eq(plugin_version.id))
        .order_by_asc(plugin_components::Column::Kind)
        .order_by_asc(plugin_components::Column::Name)
        .all(db)
        .await
    {
        Ok(c) => c,
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    let mut grouped: std::collections::BTreeMap<String, Vec<serde_json::Value>> =
        std::collections::BTreeMap::new();
    for c in components {
        grouped
            .entry(c.kind.clone())
            .or_default()
            .push(serde_json::json!({
                "name": c.name,
                "description": c.description,
                "path": c.path
            }));
    }

    Json(ApiResponse::success(serde_json::json!({
        "plugin": plugin,
        "version": plugin_version,
        "components": grouped,
        "registry": registry
    })))
}

pub async fn list_repo_skills(
    State(state): State<Arc<AppState>>,
    Path((host, org, repo)): Path<(String, String, String)>,
) -> Json<ApiResponse<Vec<SkillSummaryDto>>> {
    let db = &state.db;
    let registry = match find_registry_by_host(db, &host, &org, &repo).await {
        Ok(Some(r)) => r,
        Ok(None) => return Json(ApiResponse::error(404, "Repository not found".to_string())),
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    let mut out: Vec<SkillSummaryDto> = Vec::new();

    let standalone = match Skills::find()
        .filter(skills::Column::SkillRegistryId.eq(registry.id))
        .filter(skills::Column::IsActive.eq(1))
        .order_by_asc(skills::Column::Name)
        .all(db)
        .await
    {
        Ok(v) => v,
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    for s in standalone {
        let desc = if let Some(v) = &s.latest_version {
            SkillVersions::find()
                .filter(skill_versions::Column::SkillId.eq(s.id))
                .filter(skill_versions::Column::Version.eq(v))
                .one(db)
                .await
                .ok()
                .flatten()
                .and_then(|sv| sv.description)
        } else {
            None
        };

        out.push(SkillSummaryDto {
            name: s.name.clone(),
            description: desc,
            origin: SkillOriginDto::Standalone {
                latest_version: s.latest_version.clone(),
                ref_api: format!("/api/skills/{}/{}/{}", org, repo, s.name),
            },
        });
    }

    let plugins = match Plugins::find()
        .filter(plugins::Column::SkillRegistryId.eq(registry.id))
        .filter(plugins::Column::IsActive.eq(1))
        .all(db)
        .await
    {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    for p in plugins {
        let Some(v) = &p.latest_version else {
            continue;
        };
        let plugin_version = match PluginVersions::find()
            .filter(plugin_versions::Column::PluginId.eq(p.id))
            .filter(plugin_versions::Column::Version.eq(v))
            .one(db)
            .await
        {
            Ok(Some(v)) => v,
            _ => continue,
        };
        let comps = match PluginComponents::find()
            .filter(plugin_components::Column::PluginVersionId.eq(plugin_version.id))
            .filter(plugin_components::Column::Kind.eq("skill"))
            .order_by_asc(plugin_components::Column::Name)
            .all(db)
            .await
        {
            Ok(c) => c,
            Err(_) => continue,
        };

        for c in comps {
            out.push(SkillSummaryDto {
                name: c.name.clone(),
                description: c.description.clone(),
                origin: SkillOriginDto::Plugin {
                    plugin_name: p.name.clone(),
                    plugin_version: Some(plugin_version.version.clone()),
                    ref_api: format!(
                        "/api/{}/{}/{}/plugin/{}/skill/{}",
                        host, org, repo, p.name, c.name
                    ),
                },
            });
        }
    }

    Json(ApiResponse::success(out))
}

async fn get_repo_plugin_component(
    db: &DatabaseConnection,
    host: &str,
    org: &str,
    repo: &str,
    plugin_name: &str,
    kind: &str,
    component_name: &str,
) -> Result<serde_json::Value, ApiResponse<serde_json::Value>> {
    let registry = find_registry_by_host(db, host, org, repo)
        .await
        .map_err(|e| ApiResponse::error(500, e.to_string()))?
        .ok_or_else(|| ApiResponse::error(404, "Repository not found".to_string()))?;

    let plugin = Plugins::find()
        .filter(plugins::Column::SkillRegistryId.eq(registry.id))
        .filter(plugins::Column::Name.eq(plugin_name))
        .filter(plugins::Column::IsActive.eq(1))
        .one(db)
        .await
        .map_err(|e| ApiResponse::error(500, e.to_string()))?
        .ok_or_else(|| ApiResponse::error(404, "Plugin not found".to_string()))?;

    let latest_version = plugin
        .latest_version
        .clone()
        .ok_or_else(|| ApiResponse::error(404, "Plugin version not found".to_string()))?;

    let plugin_version = PluginVersions::find()
        .filter(plugin_versions::Column::PluginId.eq(plugin.id))
        .filter(plugin_versions::Column::Version.eq(&latest_version))
        .one(db)
        .await
        .map_err(|e| ApiResponse::error(500, e.to_string()))?
        .ok_or_else(|| ApiResponse::error(404, "Plugin version not found".to_string()))?;

    let comps = PluginComponents::find()
        .filter(plugin_components::Column::PluginVersionId.eq(plugin_version.id))
        .filter(plugin_components::Column::Kind.eq(kind))
        .filter(plugin_components::Column::Name.eq(component_name))
        .all(db)
        .await
        .map_err(|e| ApiResponse::error(500, e.to_string()))?;

    if comps.is_empty() {
        return Err(ApiResponse::error(404, "Component not found".to_string()));
    }
    if comps.len() > 1 {
        let paths: Vec<String> = comps.into_iter().map(|c| c.path).collect();
        return Err(ApiResponse {
            code: 409,
            message: "Component name is not unique".to_string(),
            data: Some(serde_json::json!({ "paths": paths })),
            timestamp: chrono::Utc::now().timestamp_millis(),
        });
    }
    let component = comps.into_iter().next().unwrap();

    Ok(serde_json::json!({
        "plugin": plugin,
        "version": plugin_version,
        "component": component,
        "registry": registry
    }))
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
    let db = &state.db;
    match get_repo_plugin_component(db, &host, &org, &repo, &plugin_name, "agent", &agent_name)
        .await
    {
        Ok(v) => Json(ApiResponse::success(v)),
        Err(resp) => Json(resp),
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
    let db = &state.db;
    match get_repo_plugin_component(db, &host, &org, &repo, &plugin_name, "skill", &skill_name)
        .await
    {
        Ok(v) => Json(ApiResponse::success(v)),
        Err(resp) => Json(resp),
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
    let db = &state.db;
    match get_repo_plugin_component(
        db,
        &host,
        &org,
        &repo,
        &plugin_name,
        "command",
        &command_name,
    )
    .await
    {
        Ok(v) => Json(ApiResponse::success(v)),
        Err(resp) => Json(resp),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::MigratorTrait;
    use sea_orm::{Database, Set};

    fn test_settings(db_url: &str) -> common::settings::Settings {
        common::settings::Settings {
            port: 0,
            database: common::settings::DatabaseSettings {
                url: db_url.to_string(),
            },
            s3: common::settings::S3Settings {
                bucket: "test".to_string(),
                region: "test".to_string(),
                endpoint: None,
                access_key_id: None,
                secret_access_key: None,
                force_path_style: false,
            },
            github: common::settings::GithubSettings {
                search_keywords: "topic:agent-skill".to_string(),
                token: None,
                api_url: "https://api.github.com".to_string(),
            },
            worker: common::settings::WorkerSettings {
                scan_interval_seconds: 60,
            },
            temporal: common::settings::TemporalSettings {
                server_url: "http://localhost:7233".to_string(),
                task_queue: "test".to_string(),
            },
            auth: Default::default(),
            debug: true,
        }
    }

    #[tokio::test]
    async fn list_repo_skills_includes_standalone_and_plugin_skills() -> anyhow::Result<()> {
        let db = Database::connect("sqlite::memory:").await?;
        migration::Migrator::up(&db, None).await?;

        let now = chrono::Utc::now().naive_utc();
        let registry = skill_registry::ActiveModel {
            platform: Set(skill_registry::Platform::Github),
            owner: Set("acme".to_string()),
            name: Set("repo".to_string()),
            url: Set("https://github.com/acme/repo".to_string()),
            description: Set(None),
            repo_type: Set(Some("marketplace".to_string())),
            status: Set("active".to_string()),
            blacklist_reason: Set(None),
            blacklisted_at: Set(None),
            stars: Set(0),
            last_scanned_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await?;

        let skill = skills::ActiveModel {
            name: Set("standalone-skill".to_string()),
            skill_registry_id: Set(registry.id),
            latest_version: Set(Some("1.0.0".to_string())),
            is_active: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await?;

        let _ = skill_versions::ActiveModel {
            skill_id: Set(skill.id),
            version: Set("1.0.0".to_string()),
            description: Set(Some("standalone desc".to_string())),
            readme_content: Set(Some("# standalone".to_string())),
            s3_key: Set(None),
            oss_url: Set(None),
            file_hash: Set(None),
            metadata: Set(None),
            created_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await?;

        let plugin = plugins::ActiveModel {
            skill_registry_id: Set(registry.id),
            name: Set("p1".to_string()),
            description: Set(Some("plugin".to_string())),
            source: Set(None),
            strict: Set(1),
            latest_version: Set(Some("1.2.3".to_string())),
            is_active: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await?;

        let plugin_version = plugin_versions::ActiveModel {
            plugin_id: Set(plugin.id),
            version: Set("1.2.3".to_string()),
            description: Set(None),
            readme_content: Set(None),
            s3_key: Set(None),
            oss_url: Set(None),
            file_hash: Set(None),
            metadata: Set(None),
            created_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await?;

        let _ = plugin_components::ActiveModel {
            plugin_version_id: Set(plugin_version.id),
            kind: Set("skill".to_string()),
            path: Set("skills/s1/SKILL.md".to_string()),
            name: Set("plugin-skill".to_string()),
            description: Set(Some("plugin skill desc".to_string())),
            markdown_content: Set(Some("# plugin skill".to_string())),
            metadata: Set(None),
            created_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await?;

        let state = Arc::new(crate::AppState {
            db,
            settings: test_settings("sqlite::memory:"),
        });

        let Json(resp) = list_repo_skills(
            State(state),
            Path((
                "github.com".to_string(),
                "acme".to_string(),
                "repo".to_string(),
            )),
        )
        .await;

        assert_eq!(resp.code, 200);
        let skills = resp.data.unwrap();
        assert_eq!(skills.len(), 2);

        Ok(())
    }
}
