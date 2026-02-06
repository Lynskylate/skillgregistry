use crate::entities::{prelude::*, skill_registry, skill_versions, skills};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use std::sync::Arc;

pub struct SkillWithRegistry {
    pub skill: skills::Model,
    pub registry: skill_registry::Model,
}

pub struct ListSkillsParams<'a> {
    pub owner: Option<&'a str>,
    pub repo: Option<&'a str>,
    pub query: Option<&'a str>,
    pub sort_by: Option<&'a str>,
    pub order: Option<&'a str>,
    pub page: u64,
    pub per_page: u64,
}

pub struct UpsertSkillParams<'a> {
    pub existing: Option<skills::Model>,
    pub skill_registry_id: i32,
    pub name: &'a str,
    pub latest_version: Option<String>,
    pub is_active: i32,
}

pub struct UpsertSkillVersionParams<'a> {
    pub existing: Option<skill_versions::Model>,
    pub skill_id: i32,
    pub version: &'a str,
    pub description: Option<String>,
    pub readme_content: Option<String>,
    pub s3_key: Option<String>,
    pub oss_url: Option<String>,
    pub file_hash: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[async_trait::async_trait]
pub trait SkillRepository: Send + Sync {
    async fn list_skills(
        &self,
        params: ListSkillsParams<'_>,
    ) -> Result<Vec<SkillWithRegistry>, DbErr>;

    async fn find_by_registry_name(
        &self,
        registry_id: i32,
        name: &str,
    ) -> Result<Option<skills::Model>, DbErr>;

    async fn find_versions(&self, skill_id: i32) -> Result<Vec<skill_versions::Model>, DbErr>;

    async fn find_version_by_name(
        &self,
        skill_id: i32,
        version: &str,
    ) -> Result<Option<skill_versions::Model>, DbErr>;

    async fn list_standalone_skills(&self, registry_id: i32) -> Result<Vec<skills::Model>, DbErr>;

    async fn list_active_skills_in_registry(
        &self,
        registry_id: i32,
    ) -> Result<Vec<skills::Model>, DbErr>;

    async fn find_skill(
        &self,
        skill_registry_id: i32,
        name: &str,
    ) -> Result<Option<skills::Model>, DbErr>;

    async fn upsert_skill(&self, params: UpsertSkillParams<'_>) -> Result<i32, DbErr>;

    async fn upsert_skill_version(&self, params: UpsertSkillVersionParams<'_>)
        -> Result<(), DbErr>;

    async fn list_skills_by_registry_id(
        &self,
        skill_registry_id: i32,
    ) -> Result<Vec<skills::Model>, DbErr>;

    async fn update_skill_active(&self, skill: skills::Model, is_active: i32) -> Result<(), DbErr>;
}

pub struct SkillRepositoryImpl {
    db: Arc<DatabaseConnection>,
}

impl SkillRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl SkillRepository for SkillRepositoryImpl {
    async fn list_skills(
        &self,
        params: ListSkillsParams<'_>,
    ) -> Result<Vec<SkillWithRegistry>, DbErr> {
        let mut query_builder = Skills::find()
            .filter(skills::Column::IsActive.eq(1))
            .find_also_related(SkillRegistry)
            .filter(skill_registry::Column::Status.ne("blacklisted"));

        if let Some(owner) = params.owner {
            query_builder = query_builder.filter(skill_registry::Column::Owner.eq(owner));
        }
        if let Some(repo) = params.repo {
            query_builder = query_builder.filter(skill_registry::Column::Name.eq(repo));
        }
        if let Some(query_str) = params.query {
            query_builder = query_builder.filter(skills::Column::Name.contains(query_str));
        }

        let query = match params.sort_by {
            Some("name") => {
                if params.order == Some("desc") {
                    query_builder.order_by_desc(skills::Column::Name)
                } else {
                    query_builder.order_by_asc(skills::Column::Name)
                }
            }
            _ => query_builder.order_by_desc(skills::Column::CreatedAt),
        };

        let paginator = query.paginate(self.db.as_ref(), params.per_page);
        let items = paginator.fetch_page(params.page.saturating_sub(1)).await?;

        let mut out = Vec::new();
        for (skill, registry_opt) in items {
            if let Some(registry) = registry_opt {
                out.push(SkillWithRegistry { skill, registry });
            }
        }
        Ok(out)
    }

    async fn find_by_registry_name(
        &self,
        registry_id: i32,
        name: &str,
    ) -> Result<Option<skills::Model>, DbErr> {
        Skills::find()
            .filter(skills::Column::SkillRegistryId.eq(registry_id))
            .filter(skills::Column::Name.eq(name))
            .filter(skills::Column::IsActive.eq(1))
            .one(self.db.as_ref())
            .await
    }

    async fn find_versions(&self, skill_id: i32) -> Result<Vec<skill_versions::Model>, DbErr> {
        SkillVersions::find()
            .filter(skill_versions::Column::SkillId.eq(skill_id))
            .all(self.db.as_ref())
            .await
    }

    async fn find_version_by_name(
        &self,
        skill_id: i32,
        version: &str,
    ) -> Result<Option<skill_versions::Model>, DbErr> {
        SkillVersions::find()
            .filter(skill_versions::Column::SkillId.eq(skill_id))
            .filter(skill_versions::Column::Version.eq(version))
            .one(self.db.as_ref())
            .await
    }

    async fn list_standalone_skills(&self, registry_id: i32) -> Result<Vec<skills::Model>, DbErr> {
        Skills::find()
            .filter(skills::Column::SkillRegistryId.eq(registry_id))
            .filter(skills::Column::IsActive.eq(1))
            .order_by_asc(skills::Column::Name)
            .all(self.db.as_ref())
            .await
    }

    async fn list_active_skills_in_registry(
        &self,
        registry_id: i32,
    ) -> Result<Vec<skills::Model>, DbErr> {
        Skills::find()
            .filter(skills::Column::SkillRegistryId.eq(registry_id))
            .filter(skills::Column::IsActive.eq(1))
            .all(self.db.as_ref())
            .await
    }

    async fn find_skill(
        &self,
        skill_registry_id: i32,
        name: &str,
    ) -> Result<Option<skills::Model>, DbErr> {
        Ok(Skills::find()
            .filter(skills::Column::SkillRegistryId.eq(skill_registry_id))
            .filter(skills::Column::Name.eq(name))
            .one(self.db.as_ref())
            .await?)
    }

    async fn upsert_skill(&self, params: UpsertSkillParams<'_>) -> Result<i32, DbErr> {
        let now = chrono::Utc::now().naive_utc();
        if let Some(s) = params.existing {
            let mut active: skills::ActiveModel = s.into();
            active.updated_at = Set(now);
            active.latest_version = Set(params.latest_version);
            active.is_active = Set(params.is_active);
            Ok(active.update(self.db.as_ref()).await?.id)
        } else {
            let new_skill = skills::ActiveModel {
                name: Set(params.name.to_string()),
                skill_registry_id: Set(params.skill_registry_id),
                latest_version: Set(params.latest_version),
                is_active: Set(params.is_active),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            };
            Ok(new_skill.insert(self.db.as_ref()).await?.id)
        }
    }

    async fn upsert_skill_version(
        &self,
        params: UpsertSkillVersionParams<'_>,
    ) -> Result<(), DbErr> {
        if let Some(v) = params.existing {
            let mut active: skill_versions::ActiveModel = v.into();
            active.description = Set(params.description);
            active.readme_content = Set(params.readme_content);
            active.s3_key = Set(params.s3_key);
            active.oss_url = Set(params.oss_url);
            active.file_hash = Set(params.file_hash);
            active.metadata = Set(params.metadata);
            let _ = active.update(self.db.as_ref()).await?;
            Ok(())
        } else {
            let new_version = skill_versions::ActiveModel {
                skill_id: Set(params.skill_id),
                version: Set(params.version.to_string()),
                description: Set(params.description),
                readme_content: Set(params.readme_content),
                s3_key: Set(params.s3_key),
                oss_url: Set(params.oss_url),
                file_hash: Set(params.file_hash),
                metadata: Set(params.metadata),
                created_at: Set(chrono::Utc::now().naive_utc()),
                ..Default::default()
            };
            let _ = new_version.insert(self.db.as_ref()).await?;
            Ok(())
        }
    }

    async fn list_skills_by_registry_id(
        &self,
        skill_registry_id: i32,
    ) -> Result<Vec<skills::Model>, DbErr> {
        Ok(Skills::find()
            .filter(skills::Column::SkillRegistryId.eq(skill_registry_id))
            .all(self.db.as_ref())
            .await?)
    }

    async fn update_skill_active(&self, skill: skills::Model, is_active: i32) -> Result<(), DbErr> {
        let mut active: skills::ActiveModel = skill.into();
        active.is_active = Set(is_active);
        active.updated_at = Set(chrono::Utc::now().naive_utc());
        let _ = active.update(self.db.as_ref()).await?;
        Ok(())
    }
}
