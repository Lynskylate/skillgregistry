use crate::entities::{blacklist, skill_registry};
use crate::repositories::registry::RegistryRepository;
use async_trait::async_trait;
use sea_orm::DbErr;
use std::sync::Arc;

#[async_trait]
pub trait RegistryService: Send + Sync {
    async fn find_by_owner_repo(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Option<skill_registry::Model>, DbErr>;

    async fn find_by_host(
        &self,
        host: &str,
        org: &str,
        repo: &str,
    ) -> Result<Option<skill_registry::Model>, DbErr>;

    async fn find_by_id(&self, id: i32) -> Result<Option<skill_registry::Model>, DbErr>;

    async fn find_all_pending(&self, expiry_date: chrono::NaiveDateTime)
        -> Result<Vec<i32>, DbErr>;

    async fn delete_expired_blacklist_entries(
        &self,
        expiry_date: chrono::NaiveDateTime,
    ) -> Result<(), DbErr>;

    async fn get_blacklist_urls(&self) -> Result<Vec<String>, DbErr>;

    async fn find_blacklist_by_url(&self, url: &str) -> Result<Option<blacklist::Model>, DbErr>;

    async fn upsert_blacklist(&self, url: &str, reason: &str) -> Result<(), DbErr>;

    async fn update_repo_blacklisted(
        &self,
        repo: &skill_registry::Model,
        reason: &str,
    ) -> Result<(), DbErr>;

    async fn update_repo_unblacklisted(
        &self,
        repo: &skill_registry::Model,
        repo_type: Option<&str>,
    ) -> Result<(), DbErr>;
}

pub struct RegistryServiceImpl {
    repo: Arc<dyn RegistryRepository>,
}

impl RegistryServiceImpl {
    pub fn new(repo: Arc<dyn RegistryRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait::async_trait]
impl RegistryService for RegistryServiceImpl {
    async fn find_by_owner_repo(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Option<skill_registry::Model>, DbErr> {
        self.repo.find_by_owner_repo(owner, repo).await
    }

    async fn find_by_host(
        &self,
        host: &str,
        org: &str,
        repo: &str,
    ) -> Result<Option<skill_registry::Model>, DbErr> {
        self.repo.find_by_host(host, org, repo).await
    }

    async fn find_by_id(&self, id: i32) -> Result<Option<skill_registry::Model>, DbErr> {
        self.repo.find_by_id(id).await
    }

    async fn find_all_pending(
        &self,
        expiry_date: chrono::NaiveDateTime,
    ) -> Result<Vec<i32>, DbErr> {
        self.repo.find_all_pending(expiry_date).await
    }

    async fn delete_expired_blacklist_entries(
        &self,
        expiry_date: chrono::NaiveDateTime,
    ) -> Result<(), DbErr> {
        self.repo
            .delete_expired_blacklist_entries(expiry_date)
            .await
    }

    async fn get_blacklist_urls(&self) -> Result<Vec<String>, DbErr> {
        self.repo.get_blacklist_urls().await
    }

    async fn find_blacklist_by_url(&self, url: &str) -> Result<Option<blacklist::Model>, DbErr> {
        self.repo.find_blacklist_by_url(url).await
    }

    async fn upsert_blacklist(&self, url: &str, reason: &str) -> Result<(), DbErr> {
        self.repo.upsert_blacklist(url, reason).await
    }

    async fn update_repo_blacklisted(
        &self,
        repo: &skill_registry::Model,
        reason: &str,
    ) -> Result<(), DbErr> {
        self.repo.update_repo_blacklisted(repo, reason).await
    }

    async fn update_repo_unblacklisted(
        &self,
        repo: &skill_registry::Model,
        repo_type: Option<&str>,
    ) -> Result<(), DbErr> {
        self.repo.update_repo_unblacklisted(repo, repo_type).await
    }
}
