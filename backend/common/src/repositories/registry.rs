use crate::entities::{blacklist, prelude::*, skill_registry};
use sea_orm::*;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait RegistryRepository: Send + Sync {
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

pub struct RegistryRepositoryImpl {
    db: Arc<DatabaseConnection>,
}

impl RegistryRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl RegistryRepository for RegistryRepositoryImpl {
    async fn find_by_owner_repo(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Option<skill_registry::Model>, DbErr> {
        SkillRegistry::find()
            .filter(skill_registry::Column::Owner.eq(owner))
            .filter(skill_registry::Column::Name.eq(repo))
            .filter(skill_registry::Column::Status.ne("blacklisted"))
            .one(self.db.as_ref())
            .await
    }

    async fn find_by_host(
        &self,
        host: &str,
        org: &str,
        repo: &str,
    ) -> Result<Option<skill_registry::Model>, DbErr> {
        let url_https = format!("https://{}/%", host);
        let url_http = format!("http://{}/%", host);

        SkillRegistry::find()
            .filter(skill_registry::Column::Owner.eq(org))
            .filter(skill_registry::Column::Name.eq(repo))
            .filter(skill_registry::Column::Status.ne("blacklisted"))
            .filter(
                Condition::any()
                    .add(skill_registry::Column::Url.like(url_https))
                    .add(skill_registry::Column::Url.like(url_http)),
            )
            .one(self.db.as_ref())
            .await
    }

    async fn find_by_id(&self, id: i32) -> Result<Option<skill_registry::Model>, DbErr> {
        SkillRegistry::find_by_id(id).one(self.db.as_ref()).await
    }

    async fn find_all_pending(
        &self,
        expiry_date: chrono::NaiveDateTime,
    ) -> Result<Vec<i32>, DbErr> {
        Self::delete_expired_blacklist_entries(self, expiry_date).await?;
        let blacklist_urls = Self::get_blacklist_urls(self).await?;

        let mut query =
            SkillRegistry::find().filter(skill_registry::Column::Status.ne("blacklisted"));

        if !blacklist_urls.is_empty() {
            query = query.filter(skill_registry::Column::Url.is_not_in(blacklist_urls));
        }

        let repos = query.all(self.db.as_ref()).await?;
        Ok(repos.into_iter().map(|r| r.id).collect())
    }

    async fn delete_expired_blacklist_entries(
        &self,
        expiry_date: chrono::NaiveDateTime,
    ) -> Result<(), DbErr> {
        let expired_entries = Blacklist::find()
            .filter(blacklist::Column::CreatedAt.lt(expiry_date))
            .all(self.db.as_ref())
            .await?;

        if !expired_entries.is_empty() {
            let expired_urls: Vec<String> = expired_entries
                .iter()
                .map(|b| b.repository_url.clone())
                .collect();

            Blacklist::delete_many()
                .filter(blacklist::Column::CreatedAt.lt(expiry_date))
                .exec(self.db.as_ref())
                .await?;

            for url in expired_urls {
                if let Some(repo) = SkillRegistry::find()
                    .filter(skill_registry::Column::Url.eq(url))
                    .one(self.db.as_ref())
                    .await?
                {
                    if repo.status == "blacklisted" {
                        Self::update_repo_unblacklisted(self, &repo, None).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn get_blacklist_urls(&self) -> Result<Vec<String>, DbErr> {
        let entries = Blacklist::find().all(self.db.as_ref()).await?;
        Ok(entries.into_iter().map(|b| b.repository_url).collect())
    }

    async fn find_blacklist_by_url(&self, url: &str) -> Result<Option<blacklist::Model>, DbErr> {
        Blacklist::find()
            .filter(blacklist::Column::RepositoryUrl.eq(url))
            .one(self.db.as_ref())
            .await
    }

    async fn upsert_blacklist(&self, url: &str, reason: &str) -> Result<(), DbErr> {
        if let Some(_existing) = Blacklist::find()
            .filter(blacklist::Column::RepositoryUrl.eq(url))
            .one(self.db.as_ref())
            .await?
        {
            Ok(())
        } else {
            let blacklist_entry = blacklist::ActiveModel {
                repository_url: Set(url.to_string()),
                reason: Set(reason.to_string()),
                created_at: Set(chrono::Utc::now().naive_utc()),
                ..Default::default()
            };
            blacklist_entry.insert(self.db.as_ref()).await?;
            Ok(())
        }
    }

    async fn update_repo_blacklisted(
        &self,
        repo: &skill_registry::Model,
        reason: &str,
    ) -> Result<(), DbErr> {
        let mut active: skill_registry::ActiveModel = repo.clone().into();
        active.status = Set("blacklisted".to_string());
        active.blacklist_reason = Set(Some(reason.to_string()));
        active.blacklisted_at = Set(Some(chrono::Utc::now().naive_utc()));
        active.update(self.db.as_ref()).await?;
        Ok(())
    }

    async fn update_repo_unblacklisted(
        &self,
        repo: &skill_registry::Model,
        repo_type: Option<&str>,
    ) -> Result<(), DbErr> {
        let mut active: skill_registry::ActiveModel = repo.clone().into();
        active.status = Set("active".to_string());
        active.repo_type = Set(repo_type.map(|s| s.to_string()));
        active.blacklist_reason = Set(None);
        active.blacklisted_at = Set(None);
        active.updated_at = Set(chrono::Utc::now().naive_utc());
        active.update(self.db.as_ref()).await?;
        Ok(())
    }
}
