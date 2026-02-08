use crate::ports::GithubApi;
use anyhow::Result;
use common::entities::{prelude::*, *};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use temporalio_sdk::ActivityError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiscoveryResult {
    pub new_count: u32,
    pub updated_count: u32,
    pub touched_repo_ids: Vec<i32>,
}

fn extract_repo_host(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

pub struct DiscoveryActivities {
    db: Arc<DatabaseConnection>,
    github: Arc<dyn GithubApi>,
    discovery_registry_service:
        Option<Arc<dyn common::services::discovery_registries::DiscoveryRegistryService>>,
}

impl DiscoveryActivities {
    pub fn new(db: Arc<DatabaseConnection>, github: Arc<dyn GithubApi>) -> Self {
        Self {
            db,
            github,
            discovery_registry_service: None,
        }
    }

    pub fn with_registry_service(
        mut self,
        discovery_registry_service: Arc<
            dyn common::services::discovery_registries::DiscoveryRegistryService,
        >,
    ) -> Self {
        self.discovery_registry_service = Some(discovery_registry_service);
        self
    }

    pub async fn discover_repos(
        &self,
        queries: Vec<String>,
    ) -> Result<DiscoveryResult, ActivityError> {
        self.discover_repos_inner(None, queries, self.github.as_ref())
            .await
            .map_err(ActivityError::from)
    }

    pub async fn fetch_due_registry_ids(&self) -> Result<Vec<i32>, ActivityError> {
        let Some(service) = self.discovery_registry_service.as_ref() else {
            return Err(ActivityError::from(anyhow::anyhow!(
                "discovery registry service is not configured"
            )));
        };

        let due = service
            .find_due(chrono::Utc::now().naive_utc())
            .await
            .map_err(ActivityError::from)?;
        Ok(due.into_iter().map(|r| r.id).collect())
    }

    pub async fn run_registry_discovery(
        &self,
        registry_id: i32,
    ) -> Result<DiscoveryResult, ActivityError> {
        let Some(service) = self.discovery_registry_service.as_ref() else {
            return Err(ActivityError::from(anyhow::anyhow!(
                "discovery registry service is not configured"
            )));
        };

        let config = service
            .find_by_id(registry_id)
            .await
            .map_err(ActivityError::from)?
            .ok_or_else(|| ActivityError::from(anyhow::anyhow!("registry not found")))?;

        let github = crate::github::GithubClient::new(Some(config.token), config.api_url.clone())
            .map_err(ActivityError::from)?;
        let result = self
            .discover_repos_inner(Some(registry_id), config.queries, &github)
            .await
            .map_err(ActivityError::from)?;

        let now = chrono::Utc::now().naive_utc();
        let interval = std::cmp::max(config.schedule_interval_seconds, 60);
        let next_run_at = now + chrono::Duration::seconds(interval);
        service
            .mark_run(
                registry_id,
                now,
                next_run_at,
                Some("success".to_string()),
                Some("Discovery workflow completed".to_string()),
            )
            .await
            .map_err(ActivityError::from)?;

        Ok(result)
    }

    async fn discover_repos_inner(
        &self,
        discovery_registry_id: Option<i32>,
        queries: Vec<String>,
        github: &dyn GithubApi,
    ) -> Result<DiscoveryResult> {
        tracing::info!("Starting discovery task...");

        let mut new_count = 0;
        let mut updated_count = 0;
        let mut touched_repo_ids = Vec::new();
        let mut processed_repos = HashSet::new();

        for query in &queries {
            tracing::info!("Searching for query: {}", query);

            let repos_result = if query.contains("filename:")
                || query.contains("path:")
                || query.contains("extension:")
            {
                github.search_code(query).await
            } else {
                let q = if !query.contains("sort:") {
                    format!("{} fork:false sort:updated", query)
                } else {
                    query.to_string()
                };
                github.search_repositories(&q).await
            };

            match repos_result {
                Ok(repos) => {
                    tracing::info!("Found {} repositories for query '{}'", repos.len(), query);

                    for repo in repos {
                        let repo_key = format!("{}/{}", repo.owner.login, repo.name);
                        if processed_repos.contains(&repo_key) {
                            continue;
                        }
                        processed_repos.insert(repo_key.clone());

                        // Check blacklist
                        let blacklisted = Blacklist::find()
                            .filter(blacklist::Column::RepositoryUrl.eq(&repo.html_url))
                            .one(&*self.db)
                            .await?;

                        if let Some(b) = blacklisted {
                            tracing::info!(
                                "Skipping blacklisted repo: {} (Reason: {})",
                                repo_key,
                                b.reason
                            );
                            continue;
                        }

                        let repo_host = extract_repo_host(&repo.html_url);

                        // Check if exists
                        let mut existing_query = SkillRegistry::find()
                            .filter(skill_registry::Column::Name.eq(&repo.name))
                            .filter(skill_registry::Column::Owner.eq(&repo.owner.login));

                        if let Some(host) = repo_host.as_deref() {
                            existing_query = existing_query.filter(
                                Condition::any()
                                    .add(skill_registry::Column::Host.eq(host))
                                    .add(
                                        skill_registry::Column::Url
                                            .like(format!("https://{}/%", host)),
                                    )
                                    .add(
                                        skill_registry::Column::Url
                                            .like(format!("http://{}/%", host)),
                                    ),
                            );
                        }

                        let existing = existing_query.one(&*self.db).await?;

                        if let Some(existing_model) = existing {
                            // Update existing
                            let mut active: skill_registry::ActiveModel = existing_model.into();
                            active.stars = Set(repo.stargazers_count);
                            active.updated_at = Set(repo.updated_at.naive_utc());
                            active.last_scanned_at = Set(Some(chrono::Utc::now().naive_utc()));
                            active.host = Set(repo_host.clone());
                            if let Some(id) = discovery_registry_id {
                                active.discovery_registry_id = Set(Some(id));
                            }
                            let updated = active.update(&*self.db).await?;
                            updated_count += 1;
                            touched_repo_ids.push(updated.id);
                        } else {
                            // Insert new
                            let new_repo = skill_registry::ActiveModel {
                                discovery_registry_id: Set(discovery_registry_id),
                                platform: Set(skill_registry::Platform::Github),
                                owner: Set(repo.owner.login.clone()),
                                name: Set(repo.name.clone()),
                                url: Set(repo.html_url.clone()),
                                host: Set(repo_host.clone()),
                                description: Set(repo.description.clone()),
                                status: Set("active".to_string()),
                                stars: Set(repo.stargazers_count),
                                created_at: Set(repo.created_at.naive_utc()),
                                updated_at: Set(repo.updated_at.naive_utc()),
                                last_scanned_at: Set(Some(chrono::Utc::now().naive_utc())),
                                ..Default::default()
                            };
                            let inserted = new_repo.insert(&*self.db).await?;
                            new_count += 1;
                            touched_repo_ids.push(inserted.id);
                            tracing::info!("Discovered new repo: {}", repo_key);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Search failed for query '{}': {}", query, e);
                }
            }
        }

        tracing::info!(
            "Discovery task completed. New: {}, Updated: {}",
            new_count,
            updated_count
        );
        Ok(DiscoveryResult {
            new_count,
            updated_count,
            touched_repo_ids,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{GithubOwner, GithubRepo};
    use crate::ports::MockGithubApi;
    use common::entities::{blacklist, skill_registry};
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, Database, DatabaseBackend, EntityTrait, MockDatabase,
        MockExecResult, QueryFilter, Set,
    };

    #[tokio::test]
    async fn test_discovery_new_repo() -> Result<()> {
        let db = MockDatabase::new(DatabaseBackend::Sqlite)
            .append_query_results::<skill_registry::Model, _, _>(vec![
                vec![], // Blacklist check
                vec![], // Existence check
                vec![skill_registry::Model {
                    id: 1,
                    discovery_registry_id: None,
                    platform: skill_registry::Platform::Github,
                    owner: "test-owner".to_string(),
                    name: "test-repo".to_string(),
                    url: "https://github.com/test-owner/test-repo".to_string(),
                    host: Some("github.com".to_string()),
                    description: Some("test description".to_string()),
                    repo_type: None,
                    status: "active".to_string(),
                    blacklist_reason: None,
                    blacklisted_at: None,
                    stars: 10,
                    created_at: chrono::Utc::now().naive_utc(),
                    updated_at: chrono::Utc::now().naive_utc(),
                    last_scanned_at: Some(chrono::Utc::now().naive_utc()),
                }], // Result of the SELECT after INSERT (SeaORM usually returns the model)
            ])
            .append_exec_results(vec![MockExecResult {
                last_insert_id: 1,
                rows_affected: 1,
            }])
            .into_connection();

        let mut github = MockGithubApi::new();

        github.expect_search_repositories().returning(|_| {
            Ok(vec![GithubRepo {
                name: "test-repo".to_string(),
                owner: GithubOwner {
                    login: "test-owner".to_string(),
                },
                html_url: "https://github.com/test-owner/test-repo".to_string(),
                description: Some("test description".to_string()),
                stargazers_count: 10,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }])
        });

        let discovery = DiscoveryActivities::new(Arc::new(db), Arc::new(github));

        let res = discovery
            .discover_repos(vec!["test-query".to_string()])
            .await
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;
        assert_eq!(res.new_count, 1);

        Ok(())
    }

    #[test]
    fn extract_repo_host_handles_invalid_and_mixed_case_urls() {
        assert_eq!(
            extract_repo_host("https://GitHub.com/acme/repo"),
            Some("github.com".to_string())
        );
        assert_eq!(
            extract_repo_host("http://example.com/repo"),
            Some("example.com".to_string())
        );
        assert_eq!(extract_repo_host("not-a-url"), None);
    }

    #[tokio::test]
    async fn discover_repos_updates_existing_and_deduplicates_results() -> Result<()> {
        use migration::MigratorTrait;

        let db = Database::connect("sqlite::memory:").await?;
        migration::Migrator::up(&db, None).await?;

        let now = chrono::Utc::now().naive_utc();
        let existing = skill_registry::ActiveModel {
            platform: Set(skill_registry::Platform::Github),
            owner: Set("acme".to_string()),
            name: Set("existing".to_string()),
            url: Set("https://github.com/acme/existing".to_string()),
            host: Set(Some("github.com".to_string())),
            status: Set("active".to_string()),
            stars: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await?;

        blacklist::ActiveModel {
            repository_url: Set("https://github.com/acme/blacklisted".to_string()),
            reason: Set("blocked".to_string()),
            created_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await?;

        let updated_repo = GithubRepo {
            name: "existing".to_string(),
            owner: GithubOwner {
                login: "acme".to_string(),
            },
            html_url: "https://github.com/acme/existing".to_string(),
            description: Some("updated".to_string()),
            stargazers_count: 99,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let new_repo = GithubRepo {
            name: "new-skill".to_string(),
            owner: GithubOwner {
                login: "acme".to_string(),
            },
            html_url: "https://github.com/acme/new-skill".to_string(),
            description: Some("new".to_string()),
            stargazers_count: 12,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let blacklisted_repo = GithubRepo {
            name: "blacklisted".to_string(),
            owner: GithubOwner {
                login: "acme".to_string(),
            },
            html_url: "https://github.com/acme/blacklisted".to_string(),
            description: Some("blocked".to_string()),
            stargazers_count: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let mut github = MockGithubApi::new();
        let updated_repo_clone = updated_repo.clone();
        let new_repo_clone = new_repo.clone();
        github
            .expect_search_repositories()
            .times(1)
            .returning(move |query| {
                assert_eq!(query, "topic:agent-skill fork:false sort:updated");
                Ok(vec![updated_repo_clone.clone(), new_repo_clone.clone()])
            });

        let new_repo_clone = new_repo.clone();
        github
            .expect_search_code()
            .times(1)
            .returning(move |query| {
                assert_eq!(query, "path:SKILL.md");
                Ok(vec![new_repo_clone.clone(), blacklisted_repo.clone()])
            });

        let discovery = DiscoveryActivities::new(Arc::new(db.clone()), Arc::new(github));
        let result = discovery
            .discover_repos(vec![
                "topic:agent-skill".to_string(),
                "path:SKILL.md".to_string(),
            ])
            .await
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;

        assert_eq!(result.new_count, 1);
        assert_eq!(result.updated_count, 1);
        assert_eq!(result.touched_repo_ids.len(), 2);
        assert!(result.touched_repo_ids.contains(&existing.id));

        let repos = SkillRegistry::find()
            .filter(skill_registry::Column::Owner.eq("acme"))
            .all(&db)
            .await?;
        assert_eq!(repos.len(), 2);

        let refreshed_existing = SkillRegistry::find_by_id(existing.id)
            .one(&db)
            .await?
            .unwrap();
        assert_eq!(refreshed_existing.stars, 99);
        assert!(refreshed_existing.last_scanned_at.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn discover_repos_swallows_search_errors_and_returns_empty_counts() -> Result<()> {
        use migration::MigratorTrait;

        let db = Database::connect("sqlite::memory:").await?;
        migration::Migrator::up(&db, None).await?;

        let mut github = MockGithubApi::new();
        github
            .expect_search_repositories()
            .times(1)
            .returning(|_| Err(anyhow::anyhow!("boom")));

        let discovery = DiscoveryActivities::new(Arc::new(db), Arc::new(github));
        let result = discovery
            .discover_repos(vec!["topic:agent-skill".to_string()])
            .await
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;

        assert_eq!(result.new_count, 0);
        assert_eq!(result.updated_count, 0);
        assert!(result.touched_repo_ids.is_empty());
        Ok(())
    }
}
