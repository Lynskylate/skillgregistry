use crate::entities::{plugin_components, plugin_versions, plugins, prelude::*, skills};
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder};
use std::sync::Arc;

#[async_trait::async_trait]
pub trait PluginRepository: Send + Sync {
    async fn list_by_registry(&self, registry_id: i32) -> Result<Vec<plugins::Model>, DbErr>;

    async fn find_by_registry_name(
        &self,
        registry_id: i32,
        name: &str,
    ) -> Result<Option<plugins::Model>, DbErr>;

    async fn find_version_by_plugin_and_version(
        &self,
        plugin_id: i32,
        version: &str,
    ) -> Result<Option<plugin_versions::Model>, DbErr>;

    async fn find_components(
        &self,
        plugin_version_id: i32,
    ) -> Result<Vec<plugin_components::Model>, DbErr>;

    async fn find_components_by_kind_name(
        &self,
        plugin_version_id: i32,
        kind: &str,
        name: &str,
    ) -> Result<Vec<plugin_components::Model>, DbErr>;

    async fn list_standalone_skills(&self, registry_id: i32) -> Result<Vec<skills::Model>, DbErr>;

    async fn list_active_plugins(&self, registry_id: i32) -> Result<Vec<plugins::Model>, DbErr>;

    async fn find_skill_components(
        &self,
        plugin_version_id: i32,
    ) -> Result<Vec<plugin_components::Model>, DbErr>;
}

pub struct PluginRepositoryImpl {
    db: Arc<DatabaseConnection>,
}

impl PluginRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl PluginRepository for PluginRepositoryImpl {
    async fn list_by_registry(&self, registry_id: i32) -> Result<Vec<plugins::Model>, DbErr> {
        Plugins::find()
            .filter(plugins::Column::SkillRegistryId.eq(registry_id))
            .filter(plugins::Column::IsActive.eq(1))
            .order_by_asc(plugins::Column::Name)
            .all(self.db.as_ref())
            .await
    }

    async fn find_by_registry_name(
        &self,
        registry_id: i32,
        name: &str,
    ) -> Result<Option<plugins::Model>, DbErr> {
        Plugins::find()
            .filter(plugins::Column::SkillRegistryId.eq(registry_id))
            .filter(plugins::Column::Name.eq(name))
            .filter(plugins::Column::IsActive.eq(1))
            .one(self.db.as_ref())
            .await
    }

    async fn find_version_by_plugin_and_version(
        &self,
        plugin_id: i32,
        version: &str,
    ) -> Result<Option<plugin_versions::Model>, DbErr> {
        PluginVersions::find()
            .filter(plugin_versions::Column::PluginId.eq(plugin_id))
            .filter(plugin_versions::Column::Version.eq(version))
            .one(self.db.as_ref())
            .await
    }

    async fn find_components(
        &self,
        plugin_version_id: i32,
    ) -> Result<Vec<plugin_components::Model>, DbErr> {
        PluginComponents::find()
            .filter(plugin_components::Column::PluginVersionId.eq(plugin_version_id))
            .order_by_asc(plugin_components::Column::Kind)
            .order_by_asc(plugin_components::Column::Name)
            .all(self.db.as_ref())
            .await
    }

    async fn find_components_by_kind_name(
        &self,
        plugin_version_id: i32,
        kind: &str,
        name: &str,
    ) -> Result<Vec<plugin_components::Model>, DbErr> {
        PluginComponents::find()
            .filter(plugin_components::Column::PluginVersionId.eq(plugin_version_id))
            .filter(plugin_components::Column::Kind.eq(kind))
            .filter(plugin_components::Column::Name.eq(name))
            .all(self.db.as_ref())
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

    async fn list_active_plugins(&self, registry_id: i32) -> Result<Vec<plugins::Model>, DbErr> {
        Plugins::find()
            .filter(plugins::Column::SkillRegistryId.eq(registry_id))
            .filter(plugins::Column::IsActive.eq(1))
            .all(self.db.as_ref())
            .await
    }

    async fn find_skill_components(
        &self,
        plugin_version_id: i32,
    ) -> Result<Vec<plugin_components::Model>, DbErr> {
        PluginComponents::find()
            .filter(plugin_components::Column::PluginVersionId.eq(plugin_version_id))
            .filter(plugin_components::Column::Kind.eq("skill"))
            .order_by_asc(plugin_components::Column::Name)
            .all(self.db.as_ref())
            .await
    }
}
