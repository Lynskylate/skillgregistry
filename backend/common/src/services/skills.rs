use super::ServiceError;
use crate::repositories::registry::RegistryRepository;
use crate::repositories::skills::{ListSkillsParams, SkillRepository};
use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;

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

#[derive(Serialize)]
pub struct SkillDetail {
    pub skill: serde_json::Value,
    pub versions: Vec<serde_json::Value>,
    pub registry: serde_json::Value,
}

#[derive(Serialize)]
pub struct SkillVersionDetail {
    pub skill_version: serde_json::Value,
}

#[async_trait]
pub trait SkillService: Send + Sync {
    async fn list_skills(
        &self,
        params: ListSkillsParams<'_>,
    ) -> Result<Vec<SkillDto>, ServiceError>;

    async fn get_skill(
        &self,
        owner: &str,
        repo: &str,
        name: &str,
    ) -> Result<SkillDetail, ServiceError>;

    async fn get_skill_version(
        &self,
        owner: &str,
        repo: &str,
        name: &str,
        version: &str,
    ) -> Result<SkillVersionDetail, ServiceError>;
}

pub struct SkillServiceImpl {
    skill_repo: Arc<dyn SkillRepository>,
    registry_repo: Arc<dyn RegistryRepository>,
}

impl SkillServiceImpl {
    pub fn new(
        skill_repo: Arc<dyn SkillRepository>,
        registry_repo: Arc<dyn RegistryRepository>,
    ) -> Self {
        Self {
            skill_repo,
            registry_repo,
        }
    }
}

#[async_trait::async_trait]
impl SkillService for SkillServiceImpl {
    async fn list_skills(
        &self,
        params: ListSkillsParams<'_>,
    ) -> Result<Vec<SkillDto>, ServiceError> {
        let items = self.skill_repo.list_skills(params).await?;

        let mut dtos = Vec::new();
        for item in items {
            let description = if let Some(ref version) = item.skill.latest_version {
                let version_opt = self
                    .skill_repo
                    .find_version_by_name(item.skill.id, version)
                    .await?;
                version_opt.and_then(|v| v.description)
            } else {
                None
            };

            dtos.push(SkillDto {
                id: item.skill.id,
                name: item.skill.name,
                owner: item.registry.owner.clone(),
                repo: item.registry.name.clone(),
                latest_version: item.skill.latest_version,
                description,
                created_at: item.skill.created_at,
            });
        }

        Ok(dtos)
    }

    async fn get_skill(
        &self,
        owner: &str,
        repo: &str,
        name: &str,
    ) -> Result<SkillDetail, ServiceError> {
        let registry = self
            .registry_repo
            .find_by_owner_repo(owner, repo)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Repository not found"))?;

        let skill = self
            .skill_repo
            .find_by_registry_name(registry.id, name)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Skill not found"))?;

        let versions = self.skill_repo.find_versions(skill.id).await?;

        Ok(SkillDetail {
            skill: serde_json::to_value(&skill)
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
            versions: versions
                .into_iter()
                .map(|v| {
                    serde_json::to_value(&v)
                        .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
            registry: serde_json::to_value(&registry)
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
        })
    }

    async fn get_skill_version(
        &self,
        owner: &str,
        repo: &str,
        name: &str,
        version: &str,
    ) -> Result<SkillVersionDetail, ServiceError> {
        let _registry = self
            .registry_repo
            .find_by_owner_repo(owner, repo)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Repository not found"))?;

        let _skill = self
            .skill_repo
            .find_by_registry_name(_registry.id, name)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Skill not found"))?;

        let skill_version = self
            .skill_repo
            .find_version_by_name(_skill.id, version)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Version not found"))?;

        Ok(SkillVersionDetail {
            skill_version: serde_json::to_value(&skill_version)
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
        })
    }
}
