use crate::entities::discovery_registries;
use crate::repositories::discovery_registries::DiscoveryRegistryRepository;
use async_trait::async_trait;
use sea_orm::DbErr;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DiscoveryRegistryConfig {
    pub id: i32,
    pub platform: discovery_registries::Platform,
    pub token: String,
    pub queries: Vec<String>,
    pub schedule_interval_seconds: i64,
    pub last_health_status: Option<String>,
    pub last_health_message: Option<String>,
    pub last_health_checked_at: Option<chrono::NaiveDateTime>,
    pub last_run_at: Option<chrono::NaiveDateTime>,
    pub next_run_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[async_trait]
pub trait DiscoveryRegistryService: Send + Sync {
    async fn list_all(&self) -> Result<Vec<DiscoveryRegistryConfig>, DbErr>;

    async fn find_by_id(&self, id: i32) -> Result<Option<DiscoveryRegistryConfig>, DbErr>;

    async fn create_github_registry(
        &self,
        token: String,
        queries: Vec<String>,
        schedule_interval_seconds: i64,
    ) -> Result<DiscoveryRegistryConfig, DbErr>;

    async fn update_config(
        &self,
        id: i32,
        queries: Vec<String>,
        schedule_interval_seconds: i64,
    ) -> Result<Option<DiscoveryRegistryConfig>, DbErr>;

    async fn delete_by_id(&self, id: i32) -> Result<bool, DbErr>;

    async fn find_due(
        &self,
        now: chrono::NaiveDateTime,
    ) -> Result<Vec<DiscoveryRegistryConfig>, DbErr>;

    async fn mark_run(
        &self,
        id: i32,
        last_run_at: chrono::NaiveDateTime,
        next_run_at: chrono::NaiveDateTime,
    ) -> Result<(), DbErr>;

    async fn update_health(
        &self,
        id: i32,
        status: String,
        message: Option<String>,
        checked_at: chrono::NaiveDateTime,
    ) -> Result<(), DbErr>;
}

pub struct DiscoveryRegistryServiceImpl {
    repo: Arc<dyn DiscoveryRegistryRepository>,
}

impl DiscoveryRegistryServiceImpl {
    pub fn new(repo: Arc<dyn DiscoveryRegistryRepository>) -> Self {
        Self { repo }
    }

    fn model_to_config(
        model: discovery_registries::Model,
    ) -> Result<DiscoveryRegistryConfig, DbErr> {
        let queries: Vec<String> = serde_json::from_str(&model.queries_json).map_err(|e| {
            DbErr::Custom(format!(
                "failed to parse discovery registry queries_json (id={}): {}",
                model.id, e
            ))
        })?;

        Ok(DiscoveryRegistryConfig {
            id: model.id,
            platform: model.platform,
            token: model.token,
            queries,
            schedule_interval_seconds: model.schedule_interval_seconds,
            last_health_status: model.last_health_status,
            last_health_message: model.last_health_message,
            last_health_checked_at: model.last_health_checked_at,
            last_run_at: model.last_run_at,
            next_run_at: model.next_run_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }

    fn serialize_queries(queries: &[String]) -> Result<String, DbErr> {
        serde_json::to_string(queries)
            .map_err(|e| DbErr::Custom(format!("failed to serialize queries: {}", e)))
    }
}

#[async_trait]
impl DiscoveryRegistryService for DiscoveryRegistryServiceImpl {
    async fn list_all(&self) -> Result<Vec<DiscoveryRegistryConfig>, DbErr> {
        self.repo
            .list_all()
            .await?
            .into_iter()
            .map(Self::model_to_config)
            .collect()
    }

    async fn find_by_id(&self, id: i32) -> Result<Option<DiscoveryRegistryConfig>, DbErr> {
        match self.repo.find_by_id(id).await? {
            Some(model) => Self::model_to_config(model).map(Some),
            None => Ok(None),
        }
    }

    async fn create_github_registry(
        &self,
        token: String,
        queries: Vec<String>,
        schedule_interval_seconds: i64,
    ) -> Result<DiscoveryRegistryConfig, DbErr> {
        let queries_json = Self::serialize_queries(&queries)?;
        let now = chrono::Utc::now().naive_utc();
        let next_run_at = now;

        let model = self
            .repo
            .create(
                discovery_registries::Platform::Github,
                token,
                queries_json,
                schedule_interval_seconds,
                now,
                next_run_at,
            )
            .await?;

        Self::model_to_config(model)
    }

    async fn update_config(
        &self,
        id: i32,
        queries: Vec<String>,
        schedule_interval_seconds: i64,
    ) -> Result<Option<DiscoveryRegistryConfig>, DbErr> {
        let queries_json = Self::serialize_queries(&queries)?;
        let now = chrono::Utc::now().naive_utc();

        let model = self
            .repo
            .update_config(id, queries_json, schedule_interval_seconds, now)
            .await?;

        model.map(Self::model_to_config).transpose()
    }

    async fn delete_by_id(&self, id: i32) -> Result<bool, DbErr> {
        self.repo.delete_by_id(id).await
    }

    async fn find_due(
        &self,
        now: chrono::NaiveDateTime,
    ) -> Result<Vec<DiscoveryRegistryConfig>, DbErr> {
        self.repo
            .find_due(now)
            .await?
            .into_iter()
            .map(Self::model_to_config)
            .collect()
    }

    async fn mark_run(
        &self,
        id: i32,
        last_run_at: chrono::NaiveDateTime,
        next_run_at: chrono::NaiveDateTime,
    ) -> Result<(), DbErr> {
        self.repo
            .update_run_timestamps(id, last_run_at, next_run_at, chrono::Utc::now().naive_utc())
            .await
    }

    async fn update_health(
        &self,
        id: i32,
        status: String,
        message: Option<String>,
        checked_at: chrono::NaiveDateTime,
    ) -> Result<(), DbErr> {
        self.repo
            .update_health(
                id,
                status,
                message,
                checked_at,
                chrono::Utc::now().naive_utc(),
            )
            .await
    }
}
