use crate::entities::{discovery_registries, prelude::DiscoveryRegistries};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CreateDiscoveryRegistryParams {
    pub platform: discovery_registries::Platform,
    pub token: String,
    pub api_url: String,
    pub queries_json: String,
    pub schedule_interval_seconds: i64,
    pub now: chrono::NaiveDateTime,
    pub next_run_at: chrono::NaiveDateTime,
}

#[async_trait::async_trait]
pub trait DiscoveryRegistryRepository: Send + Sync {
    async fn list_all(&self) -> Result<Vec<discovery_registries::Model>, DbErr>;

    async fn find_by_id(&self, id: i32) -> Result<Option<discovery_registries::Model>, DbErr>;

    async fn create(
        &self,
        params: CreateDiscoveryRegistryParams,
    ) -> Result<discovery_registries::Model, DbErr>;

    async fn update_config(
        &self,
        id: i32,
        queries_json: String,
        schedule_interval_seconds: i64,
        api_url: String,
        updated_at: chrono::NaiveDateTime,
    ) -> Result<Option<discovery_registries::Model>, DbErr>;

    async fn delete_by_id(&self, id: i32) -> Result<bool, DbErr>;

    async fn find_due(
        &self,
        now: chrono::NaiveDateTime,
    ) -> Result<Vec<discovery_registries::Model>, DbErr>;

    async fn update_run_timestamps(
        &self,
        id: i32,
        last_run_at: chrono::NaiveDateTime,
        next_run_at: chrono::NaiveDateTime,
        updated_at: chrono::NaiveDateTime,
    ) -> Result<(), DbErr>;

    async fn update_health(
        &self,
        id: i32,
        status: String,
        message: Option<String>,
        checked_at: chrono::NaiveDateTime,
        updated_at: chrono::NaiveDateTime,
    ) -> Result<(), DbErr>;
}

pub struct DiscoveryRegistryRepositoryImpl {
    db: Arc<DatabaseConnection>,
}

impl DiscoveryRegistryRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl DiscoveryRegistryRepository for DiscoveryRegistryRepositoryImpl {
    async fn list_all(&self) -> Result<Vec<discovery_registries::Model>, DbErr> {
        DiscoveryRegistries::find()
            .order_by_desc(discovery_registries::Column::CreatedAt)
            .all(self.db.as_ref())
            .await
    }

    async fn find_by_id(&self, id: i32) -> Result<Option<discovery_registries::Model>, DbErr> {
        DiscoveryRegistries::find_by_id(id)
            .one(self.db.as_ref())
            .await
    }

    async fn create(
        &self,
        params: CreateDiscoveryRegistryParams,
    ) -> Result<discovery_registries::Model, DbErr> {
        let CreateDiscoveryRegistryParams {
            platform,
            token,
            api_url,
            queries_json,
            schedule_interval_seconds,
            now,
            next_run_at,
        } = params;

        discovery_registries::ActiveModel {
            platform: Set(platform),
            token: Set(token),
            api_url: Set(api_url),
            queries_json: Set(queries_json),
            schedule_interval_seconds: Set(schedule_interval_seconds),
            last_health_status: Set(None),
            last_health_message: Set(None),
            last_health_checked_at: Set(None),
            last_run_at: Set(None),
            next_run_at: Set(Some(next_run_at)),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(self.db.as_ref())
        .await
    }

    async fn update_config(
        &self,
        id: i32,
        queries_json: String,
        schedule_interval_seconds: i64,
        api_url: String,
        updated_at: chrono::NaiveDateTime,
    ) -> Result<Option<discovery_registries::Model>, DbErr> {
        let Some(existing) = self.find_by_id(id).await? else {
            return Ok(None);
        };

        let mut active: discovery_registries::ActiveModel = existing.into();
        active.queries_json = Set(queries_json);
        active.schedule_interval_seconds = Set(schedule_interval_seconds);
        active.api_url = Set(api_url);
        active.next_run_at = Set(Some(updated_at));
        active.updated_at = Set(updated_at);

        active.update(self.db.as_ref()).await.map(Some)
    }

    async fn delete_by_id(&self, id: i32) -> Result<bool, DbErr> {
        let res = DiscoveryRegistries::delete_by_id(id)
            .exec(self.db.as_ref())
            .await?;
        Ok(res.rows_affected > 0)
    }

    async fn find_due(
        &self,
        now: chrono::NaiveDateTime,
    ) -> Result<Vec<discovery_registries::Model>, DbErr> {
        DiscoveryRegistries::find()
            .filter(discovery_registries::Column::NextRunAt.lte(now))
            .order_by_asc(discovery_registries::Column::NextRunAt)
            .all(self.db.as_ref())
            .await
    }

    async fn update_run_timestamps(
        &self,
        id: i32,
        last_run_at: chrono::NaiveDateTime,
        next_run_at: chrono::NaiveDateTime,
        updated_at: chrono::NaiveDateTime,
    ) -> Result<(), DbErr> {
        if let Some(existing) = self.find_by_id(id).await? {
            let mut active: discovery_registries::ActiveModel = existing.into();
            active.last_run_at = Set(Some(last_run_at));
            active.next_run_at = Set(Some(next_run_at));
            active.updated_at = Set(updated_at);
            active.update(self.db.as_ref()).await?;
        }

        Ok(())
    }

    async fn update_health(
        &self,
        id: i32,
        status: String,
        message: Option<String>,
        checked_at: chrono::NaiveDateTime,
        updated_at: chrono::NaiveDateTime,
    ) -> Result<(), DbErr> {
        if let Some(existing) = self.find_by_id(id).await? {
            let mut active: discovery_registries::ActiveModel = existing.into();
            active.last_health_status = Set(Some(status));
            active.last_health_message = Set(message);
            active.last_health_checked_at = Set(Some(checked_at));
            active.updated_at = Set(updated_at);
            active.update(self.db.as_ref()).await?;
        }

        Ok(())
    }
}
