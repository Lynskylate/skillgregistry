use anyhow::Result;
use sea_orm::*;
use common::entities::{prelude::*, *};
use crate::ports::GithubApi;
use std::collections::HashSet;

pub async fn run(db: &DatabaseConnection, github: &impl GithubApi, queries: Vec<String>) -> Result<()> {
    tracing::info!("Starting discovery task...");

    let mut new_count = 0;
    let mut updated_count = 0;
    let mut processed_repos = HashSet::new();

    for query in &queries {
        tracing::info!("Searching for query: {}", query);
        
        let repos_result = if query.contains("filename:") || query.contains("path:") || query.contains("extension:") {
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
                        .one(db)
                        .await?;
                    
                    if let Some(b) = blacklisted {
                        tracing::info!("Skipping blacklisted repo: {} (Reason: {})", repo_key, b.reason);
                        continue;
                    }

                    // Check if exists
                    let existing = SkillRegistry::find()
                        .filter(skill_registry::Column::Name.eq(&repo.name))
                        .filter(skill_registry::Column::Owner.eq(&repo.owner.login))
                        .one(db)
                        .await?;

                    if let Some(existing_model) = existing {
                        // Update existing
                        let mut active: skill_registry::ActiveModel = existing_model.into();
                        active.stars = Set(repo.stargazers_count);
                        active.updated_at = Set(repo.updated_at.naive_utc());
                        active.last_scanned_at = Set(Some(chrono::Utc::now().naive_utc()));
                        active.update(db).await?;
                        updated_count += 1;
                    } else {
                        // Insert new
                        // Note: If code search returned partial repo info, we might want to fetch full details
                        // But GithubCodeItem.repository seems to have most fields. 
                        // However, description might be missing or stars might be 0 in some contexts?
                        // Let's trust it for now, or fetch if critical fields missing.
                        
                        let new_repo = skill_registry::ActiveModel {
                            platform: Set(skill_registry::Platform::Github),
                            owner: Set(repo.owner.login.clone()),
                            name: Set(repo.name.clone()),
                            url: Set(repo.html_url.clone()),
                            description: Set(repo.description.clone()),
                            stars: Set(repo.stargazers_count),
                            created_at: Set(repo.created_at.naive_utc()),
                            updated_at: Set(repo.updated_at.naive_utc()),
                            last_scanned_at: Set(Some(chrono::Utc::now().naive_utc())),
                            ..Default::default()
                        };
                        new_repo.insert(db).await?;
                        new_count += 1;
                        tracing::info!("Discovered new repo: {}", repo_key);
                    }
                }
            },
            Err(e) => {
                tracing::error!("Search failed for query '{}': {}", query, e);
            }
        }
    }

    tracing::info!("Discovery task completed. New: {}, Updated: {}", new_count, updated_count);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::MockGithubApi;
    use crate::github::{GithubRepo, GithubOwner};
    use sea_orm::DatabaseBackend;
    use common::entities::skill_registry;

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
                    stars: 10,
                    created_at: chrono::Utc::now().naive_utc(),
                    updated_at: chrono::Utc::now().naive_utc(),
                    last_scanned_at: Some(chrono::Utc::now().naive_utc()),
                }], // Result of the SELECT after INSERT
            ])
            .append_exec_results(vec![
                MockExecResult {
                    last_insert_id: 1,
                    rows_affected: 1,
                },
            ])
            .into_connection();

        let mut github = MockGithubApi::new();
        
        github.expect_search_repositories()
            .returning(|_| Ok(vec![GithubRepo {
                id: 1,
                name: "test-repo".to_string(),
                full_name: "test-owner/test-repo".to_string(),
                owner: GithubOwner { login: "test-owner".to_string() },
                html_url: "https://github.com/test-owner/test-repo".to_string(),
                description: Some("test description".to_string()),
                stargazers_count: 10,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                pushed_at: chrono::Utc::now(),
                fork: false,
            }]));

        run(&db, &github, vec!["test-query".to_string()]).await?;

        // Check logs or db state if needed, but the mock db ensures queries were executed
        Ok(())
    }

    #[tokio::test]
    async fn test_discovery_update_repo() -> Result<()> {
        let db = MockDatabase::new(DatabaseBackend::Sqlite)
            .append_query_results::<skill_registry::Model, _, _>(vec![
                vec![], // Blacklist check
                vec![skill_registry::Model {         // Existence check
                    id: 1,
                    platform: skill_registry::Platform::Github,
                    owner: "test-owner".to_string(),
                    name: "test-repo".to_string(),
                    url: "https://github.com/test-owner/test-repo".to_string(),
                    description: Some("old description".to_string()),
                    stars: 5,
                    created_at: chrono::Utc::now().naive_utc(),
                    updated_at: chrono::Utc::now().naive_utc(),
                    last_scanned_at: None,
                }],
                vec![skill_registry::Model {         // Result after UPDATE
                    id: 1,
                    platform: skill_registry::Platform::Github,
                    owner: "test-owner".to_string(),
                    name: "test-repo".to_string(),
                    url: "https://github.com/test-owner/test-repo".to_string(),
                    description: Some("old description".to_string()),
                    stars: 10,
                    created_at: chrono::Utc::now().naive_utc(),
                    updated_at: chrono::Utc::now().naive_utc(),
                    last_scanned_at: None,
                }],
            ])
            .append_exec_results(vec![
                MockExecResult {
                    last_insert_id: 0,
                    rows_affected: 1,
                },
            ])
            .into_connection();

        let mut github = MockGithubApi::new();
        
        github.expect_search_repositories()
            .returning(|_| Ok(vec![GithubRepo {
                id: 1,
                name: "test-repo".to_string(),
                full_name: "test-owner/test-repo".to_string(),
                owner: GithubOwner { login: "test-owner".to_string() },
                html_url: "https://github.com/test-owner/test-repo".to_string(),
                description: Some("old description".to_string()),
                stargazers_count: 10, // Stars updated from 5 to 10
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                pushed_at: chrono::Utc::now(),
                fork: false,
            }]));

        run(&db, &github, vec!["test-query".to_string()]).await?;

        Ok(())
    }
}
