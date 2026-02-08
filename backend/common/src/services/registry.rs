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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{blacklist, skill_registry};
    use crate::repositories::registry::RegistryRepository;
    use sea_orm::DbErr;
    use std::sync::Arc;

    struct StubRegistryRepo;

    fn sample_repo() -> skill_registry::Model {
        let now = chrono::Utc::now().naive_utc();
        skill_registry::Model {
            id: 7,
            discovery_registry_id: None,
            platform: skill_registry::Platform::Github,
            owner: "acme".to_string(),
            name: "skills".to_string(),
            url: "https://github.com/acme/skills".to_string(),
            host: Some("github.com".to_string()),
            description: Some("repo".to_string()),
            repo_type: Some("skill".to_string()),
            status: "active".to_string(),
            blacklist_reason: None,
            blacklisted_at: None,
            stars: 10,
            last_scanned_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[async_trait]
    impl RegistryRepository for StubRegistryRepo {
        async fn find_by_owner_repo(
            &self,
            _owner: &str,
            _repo: &str,
        ) -> Result<Option<skill_registry::Model>, DbErr> {
            Ok(Some(sample_repo()))
        }

        async fn find_by_host(
            &self,
            _host: &str,
            _org: &str,
            _repo: &str,
        ) -> Result<Option<skill_registry::Model>, DbErr> {
            Ok(Some(sample_repo()))
        }

        async fn find_by_id(&self, _id: i32) -> Result<Option<skill_registry::Model>, DbErr> {
            Ok(Some(sample_repo()))
        }

        async fn find_all_pending(
            &self,
            _expiry_date: chrono::NaiveDateTime,
        ) -> Result<Vec<i32>, DbErr> {
            Ok(vec![7])
        }

        async fn delete_expired_blacklist_entries(
            &self,
            _expiry_date: chrono::NaiveDateTime,
        ) -> Result<(), DbErr> {
            Ok(())
        }

        async fn get_blacklist_urls(&self) -> Result<Vec<String>, DbErr> {
            Ok(vec!["https://github.com/acme/blocked".to_string()])
        }

        async fn find_blacklist_by_url(
            &self,
            url: &str,
        ) -> Result<Option<blacklist::Model>, DbErr> {
            Ok(Some(blacklist::Model {
                id: 3,
                repository_url: url.to_string(),
                reason: "blocked".to_string(),
                created_at: chrono::Utc::now().naive_utc(),
            }))
        }

        async fn upsert_blacklist(&self, _url: &str, _reason: &str) -> Result<(), DbErr> {
            Ok(())
        }

        async fn update_repo_blacklisted(
            &self,
            _repo: &skill_registry::Model,
            _reason: &str,
        ) -> Result<(), DbErr> {
            Ok(())
        }

        async fn update_repo_unblacklisted(
            &self,
            _repo: &skill_registry::Model,
            _repo_type: Option<&str>,
        ) -> Result<(), DbErr> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn registry_service_forwards_to_repository() {
        let service = RegistryServiceImpl::new(Arc::new(StubRegistryRepo));
        let now = chrono::Utc::now().naive_utc();

        assert!(service
            .find_by_owner_repo("acme", "skills")
            .await
            .unwrap()
            .is_some());
        assert!(service
            .find_by_host("github.com", "acme", "skills")
            .await
            .unwrap()
            .is_some());
        assert!(service.find_by_id(7).await.unwrap().is_some());
        assert_eq!(service.find_all_pending(now).await.unwrap(), vec![7]);
        assert!(service.delete_expired_blacklist_entries(now).await.is_ok());
        assert_eq!(
            service.get_blacklist_urls().await.unwrap(),
            vec!["https://github.com/acme/blocked".to_string()]
        );

        let blacklist = service
            .find_blacklist_by_url("https://github.com/acme/blocked")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(blacklist.reason, "blocked");

        assert!(service
            .upsert_blacklist("https://github.com/acme/blocked", "reason")
            .await
            .is_ok());
        assert!(service
            .update_repo_blacklisted(&sample_repo(), "reason")
            .await
            .is_ok());
        assert!(service
            .update_repo_unblacklisted(&sample_repo(), Some("skill"))
            .await
            .is_ok());
    }
}
