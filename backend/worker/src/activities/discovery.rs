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
}

pub struct DiscoveryActivities {
    db: Arc<DatabaseConnection>,
    github: Arc<dyn GithubApi>,
}

impl DiscoveryActivities {
    pub fn new(db: Arc<DatabaseConnection>, github: Arc<dyn GithubApi>) -> Self {
        Self { db, github }
    }

    pub async fn discover_repos(
        &self,
        queries: Vec<String>,
    ) -> Result<DiscoveryResult, ActivityError> {
        tracing::info!("Starting discovery task...");

        let mut new_count = 0;
        let mut updated_count = 0;
        let mut processed_repos = HashSet::new();

        for query in &queries {
            tracing::info!("Searching for query: {}", query);

            let repos_result = if query.contains("filename:")
                || query.contains("path:")
                || query.contains("extension:")
            {
                self.github.search_code(query).await
            } else {
                let q = if !query.contains("sort:") {
                    format!("{} fork:false sort:updated", query)
                } else {
                    query.to_string()
                };
                self.github.search_repositories(&q).await
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

                        // Check if exists
                        let existing = SkillRegistry::find()
                            .filter(skill_registry::Column::Name.eq(&repo.name))
                            .filter(skill_registry::Column::Owner.eq(&repo.owner.login))
                            .one(&*self.db)
                            .await?;

                        if let Some(existing_model) = existing {
                            // Update existing
                            let mut active: skill_registry::ActiveModel = existing_model.into();
                            active.stars = Set(repo.stargazers_count);
                            active.updated_at = Set(repo.updated_at.naive_utc());
                            active.last_scanned_at = Set(Some(chrono::Utc::now().naive_utc()));
                            active.update(&*self.db).await?;
                            updated_count += 1;
                        } else {
                            // Insert new
                            let new_repo = skill_registry::ActiveModel {
                                platform: Set(skill_registry::Platform::Github),
                                owner: Set(repo.owner.login.clone()),
                                name: Set(repo.name.clone()),
                                url: Set(repo.html_url.clone()),
                                description: Set(repo.description.clone()),
                                status: Set("active".to_string()),
                                stars: Set(repo.stargazers_count),
                                created_at: Set(repo.created_at.naive_utc()),
                                updated_at: Set(repo.updated_at.naive_utc()),
                                last_scanned_at: Set(Some(chrono::Utc::now().naive_utc())),
                                ..Default::default()
                            };
                            new_repo.insert(&*self.db).await?;
                            new_count += 1;
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{GithubOwner, GithubRepo};
    use crate::ports::MockGithubApi;
    use common::entities::skill_registry;
    use sea_orm::DatabaseBackend;
    use sea_orm::MockDatabase;
    use sea_orm::MockExecResult;

    #[tokio::test]
    async fn test_discovery_new_repo() -> Result<()> {
        let db = MockDatabase::new(DatabaseBackend::Sqlite)
            .append_query_results::<skill_registry::Model, _, _>(vec![
                vec![], // Blacklist check
                vec![], // Existence check
                vec![skill_registry::Model {
                    id: 1,
                    platform: skill_registry::Platform::Github,
                    owner: "test-owner".to_string(),
                    name: "test-repo".to_string(),
                    url: "https://github.com/test-owner/test-repo".to_string(),
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
}
