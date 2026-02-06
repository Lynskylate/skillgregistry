use super::ServiceError;
use crate::repositories::plugins::PluginRepository;
use crate::repositories::registry::RegistryRepository;
use crate::repositories::skills::SkillRepository;
use async_trait::async_trait;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

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

#[derive(Serialize)]
pub struct PluginDetail {
    pub plugin: serde_json::Value,
    pub version: serde_json::Value,
    pub components: BTreeMap<String, Vec<serde_json::Value>>,
    pub registry: serde_json::Value,
}

#[derive(Serialize)]
pub struct PluginComponentDetail {
    pub plugin: serde_json::Value,
    pub version: serde_json::Value,
    pub component: serde_json::Value,
    pub registry: serde_json::Value,
}

#[async_trait]
pub trait PluginService: Send + Sync {
    async fn list_repo_plugins(
        &self,
        host: &str,
        org: &str,
        repo: &str,
    ) -> Result<Vec<PluginListItemDto>, ServiceError>;

    async fn get_repo_plugin(
        &self,
        host: &str,
        org: &str,
        repo: &str,
        plugin_name: &str,
    ) -> Result<PluginDetail, ServiceError>;

    async fn list_repo_skills(
        &self,
        host: &str,
        org: &str,
        repo: &str,
    ) -> Result<Vec<SkillSummaryDto>, ServiceError>;

    async fn get_repo_plugin_component(
        &self,
        host: &str,
        org: &str,
        repo: &str,
        plugin_name: &str,
        kind: &str,
        component_name: &str,
    ) -> Result<PluginComponentDetail, ServiceError>;
}

pub struct PluginServiceImpl {
    plugin_repo: Arc<dyn PluginRepository>,
    registry_repo: Arc<dyn RegistryRepository>,
    skill_repo: Arc<dyn SkillRepository>,
}

impl PluginServiceImpl {
    pub fn new(
        plugin_repo: Arc<dyn PluginRepository>,
        registry_repo: Arc<dyn RegistryRepository>,
        skill_repo: Arc<dyn SkillRepository>,
    ) -> Self {
        Self {
            plugin_repo,
            registry_repo,
            skill_repo,
        }
    }
}

#[async_trait::async_trait]
impl PluginService for PluginServiceImpl {
    async fn list_repo_plugins(
        &self,
        host: &str,
        org: &str,
        repo: &str,
    ) -> Result<Vec<PluginListItemDto>, ServiceError> {
        let registry = self
            .registry_repo
            .find_by_host(host, org, repo)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Repository not found"))?;

        let plugins = self.plugin_repo.list_by_registry(registry.id).await?;

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

        Ok(dtos)
    }

    async fn get_repo_plugin(
        &self,
        host: &str,
        org: &str,
        repo: &str,
        plugin_name: &str,
    ) -> Result<PluginDetail, ServiceError> {
        let registry = self
            .registry_repo
            .find_by_host(host, org, repo)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Repository not found"))?;

        let plugin = self
            .plugin_repo
            .find_by_registry_name(registry.id, plugin_name)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Plugin not found"))?;

        let latest_version = plugin
            .latest_version
            .clone()
            .ok_or_else(|| ServiceError::new(404, "Plugin version not found"))?;

        let plugin_version = self
            .plugin_repo
            .find_version_by_plugin_and_version(plugin.id, &latest_version)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Plugin version not found"))?;

        let components = self.plugin_repo.find_components(plugin_version.id).await?;

        let mut grouped: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
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

        Ok(PluginDetail {
            plugin: serde_json::to_value(&plugin)
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
            version: serde_json::to_value(&plugin_version)
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
            components: grouped,
            registry: serde_json::to_value(&registry)
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
        })
    }

    async fn list_repo_skills(
        &self,
        host: &str,
        org: &str,
        repo: &str,
    ) -> Result<Vec<SkillSummaryDto>, ServiceError> {
        let registry = self
            .registry_repo
            .find_by_host(host, org, repo)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Repository not found"))?;

        let mut out: Vec<SkillSummaryDto> = Vec::new();

        let standalone = self.plugin_repo.list_standalone_skills(registry.id).await?;

        for s in standalone {
            let description = if let Some(ref version) = s.latest_version {
                let version_opt = self.skill_repo.find_version_by_name(s.id, version).await?;
                version_opt.and_then(|v| v.description)
            } else {
                None
            };

            out.push(SkillSummaryDto {
                name: s.name.clone(),
                description,
                origin: SkillOriginDto::Standalone {
                    latest_version: s.latest_version.clone(),
                    ref_api: format!("/api/skills/{}/{}/{}", org, repo, s.name),
                },
            });
        }

        let plugins = self.plugin_repo.list_active_plugins(registry.id).await?;

        for p in plugins {
            let Some(v) = &p.latest_version else {
                continue;
            };

            let plugin_version = self
                .plugin_repo
                .find_version_by_plugin_and_version(p.id, v)
                .await?;

            let Some(plugin_version) = plugin_version else {
                continue;
            };

            let comps = self
                .plugin_repo
                .find_skill_components(plugin_version.id)
                .await?;

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

        Ok(out)
    }

    async fn get_repo_plugin_component(
        &self,
        host: &str,
        org: &str,
        repo: &str,
        plugin_name: &str,
        kind: &str,
        component_name: &str,
    ) -> Result<PluginComponentDetail, ServiceError> {
        let registry = self
            .registry_repo
            .find_by_host(host, org, repo)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Repository not found"))?;

        let plugin = self
            .plugin_repo
            .find_by_registry_name(registry.id, plugin_name)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Plugin not found"))?;

        let latest_version = plugin
            .latest_version
            .clone()
            .ok_or_else(|| ServiceError::new(404, "Plugin version not found"))?;

        let plugin_version = self
            .plugin_repo
            .find_version_by_plugin_and_version(plugin.id, &latest_version)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Plugin version not found"))?;

        let comps = self
            .plugin_repo
            .find_components_by_kind_name(plugin_version.id, kind, component_name)
            .await?;

        if comps.is_empty() {
            return Err(ServiceError::new(404, "Component not found"));
        }
        if comps.len() > 1 {
            let paths: Vec<String> = comps.into_iter().map(|c| c.path).collect();
            return Err(ServiceError::with_data(
                ServiceError::new(409, "Component name is not unique"),
                serde_json::json!({ "paths": paths }),
            ));
        }

        let component = comps.into_iter().next().unwrap();

        Ok(PluginComponentDetail {
            plugin: serde_json::to_value(&plugin)
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
            version: serde_json::to_value(&plugin_version)
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
            component: serde_json::to_value(&component)
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
            registry: serde_json::to_value(&registry)
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
        })
    }
}
