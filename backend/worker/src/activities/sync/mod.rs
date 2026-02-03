pub mod domain;
pub mod download;
pub mod marketplace;
pub mod standalone;
pub mod utils;

pub use self::domain::SyncResult;
use self::download::zip_to_file_map;
use self::marketplace::sync_marketplace_plugins;
use self::standalone::sync_standalone_skills;
use crate::ports::{GithubApi, Storage};
use anyhow::Result;
use common::entities::{prelude::*, *};
use sea_orm::*;
use serde_json::Value;
use std::collections::HashSet;
use temporalio_sdk::{ActContext, ActivityError};

pub struct SyncService;

impl SyncService {
    pub async fn fetch_pending(db: &DatabaseConnection) -> Result<Vec<i32>> {
        let expiry_date = chrono::Utc::now().naive_utc() - chrono::Duration::days(30);
        let expired_entries = Blacklist::find()
            .filter(blacklist::Column::CreatedAt.lt(expiry_date))
            .all(db)
            .await?;
        if !expired_entries.is_empty() {
            let expired_urls: Vec<String> = expired_entries
                .iter()
                .map(|b| b.repository_url.clone())
                .collect();

            let _ = Blacklist::delete_many()
                .filter(blacklist::Column::CreatedAt.lt(expiry_date))
                .exec(db)
                .await;

            for url in expired_urls {
                if let Some(repo) = SkillRegistry::find()
                    .filter(skill_registry::Column::Url.eq(url))
                    .one(db)
                    .await?
                {
                    if repo.status == "blacklisted" {
                        let mut active: skill_registry::ActiveModel = repo.into();
                        active.status = Set("active".to_string());
                        active.repo_type = Set(None);
                        active.blacklist_reason = Set(None);
                        active.blacklisted_at = Set(None);
                        active.updated_at = Set(chrono::Utc::now().naive_utc());
                        let _ = active.update(db).await;
                    }
                }
            }
        }

        let blacklist_urls: Vec<String> = Blacklist::find()
            .all(db)
            .await?
            .into_iter()
            .map(|b| b.repository_url)
            .collect();

        let mut query =
            SkillRegistry::find().filter(skill_registry::Column::Status.ne("blacklisted"));
        if !blacklist_urls.is_empty() {
            query = query.filter(skill_registry::Column::Url.is_not_in(blacklist_urls));
        }

        let repos = query.all(db).await?;
        Ok(repos.into_iter().map(|r| r.id).collect())
    }

    pub async fn process_one(
        db: &DatabaseConnection,
        s3: &impl Storage,
        github: &impl GithubApi,
        registry_id: i32,
    ) -> Result<SyncResult> {
        let repo = SkillRegistry::find_by_id(registry_id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Registry entry not found"))?;

        tracing::info!("Processing repo: {}/{}", repo.owner, repo.name);

        if repo.status == "blacklisted" {
            return Ok(SyncResult {
                status: "SkippedBlacklisted".to_string(),
                version: None,
            });
        }

        let is_blacklisted = Blacklist::find()
            .filter(blacklist::Column::RepositoryUrl.eq(&repo.url))
            .one(db)
            .await?
            .is_some();
        if is_blacklisted {
            return Ok(SyncResult {
                status: "SkippedBlacklisted".to_string(),
                version: None,
            });
        }

        let zip_data = match github.download_zipball(&repo.owner, &repo.name).await {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!("Failed to download zip: {}", e);
                return Err(e);
            }
        };

        let file_map = match zip_to_file_map(&zip_data) {
            Ok(m) => m,
            Err(e) => {
                Self::blacklist_repo(db, &repo, &format!("Invalid zip archive: {}", e)).await?;
                return Ok(SyncResult {
                    status: "Blacklisted".to_string(),
                    version: None,
                });
            }
        };

        let mut changed = false;

        if let Some(marketplace_bytes) = file_map.get(".claude-plugin/marketplace.json") {
            let marketplace_json: Value = match serde_json::from_slice(marketplace_bytes) {
                Ok(v) => v,
                Err(e) => {
                    Self::blacklist_repo(
                        db,
                        &repo,
                        &format!("Invalid marketplace.json (JSON parse error): {}", e),
                    )
                    .await?;
                    return Ok(SyncResult {
                        status: "Blacklisted".to_string(),
                        version: None,
                    });
                }
            };

            let plugin_outcome =
                sync_marketplace_plugins(db, s3, &repo, &file_map, &marketplace_json).await?;
            changed |= plugin_outcome.changed;

            let skill_outcome = sync_standalone_skills(
                db,
                s3,
                &repo,
                &file_map,
                &plugin_outcome.plugin_root_prefixes,
                false,
            )
            .await?;
            changed |= skill_outcome.changed;

            Self::mark_repo_active(db, &repo, Some("marketplace")).await?;

            return Ok(SyncResult {
                status: if changed {
                    "Updated".to_string()
                } else {
                    "Unchanged".to_string()
                },
                version: None,
            });
        }

        let skill_outcome =
            sync_standalone_skills(db, s3, &repo, &file_map, &HashSet::new(), true).await?;
        changed |= skill_outcome.changed;

        if !skill_outcome.found_any {
            Self::blacklist_repo(db, &repo, "No valid SKILL.md found").await?;
            return Ok(SyncResult {
                status: "Blacklisted".to_string(),
                version: None,
            });
        }

        Self::mark_repo_active(db, &repo, Some("skill")).await?;

        Ok(SyncResult {
            status: if changed {
                "Updated".to_string()
            } else {
                "Unchanged".to_string()
            },
            version: None,
        })
    }

    async fn mark_repo_active(
        db: &DatabaseConnection,
        repo: &skill_registry::Model,
        repo_type: Option<&str>,
    ) -> Result<()> {
        let mut active: skill_registry::ActiveModel = repo.clone().into();
        active.status = Set("active".to_string());
        active.repo_type = Set(repo_type.map(|s| s.to_string()));
        active.blacklist_reason = Set(None);
        active.blacklisted_at = Set(None);
        active.updated_at = Set(chrono::Utc::now().naive_utc());
        let _ = active.update(db).await?;
        Ok(())
    }

    async fn blacklist_repo(
        db: &DatabaseConnection,
        repo: &skill_registry::Model,
        reason: &str,
    ) -> Result<()> {
        tracing::warn!("Blacklisting repo {}/{}: {}", repo.owner, repo.name, reason);

        if let Some(existing) = Blacklist::find()
            .filter(blacklist::Column::RepositoryUrl.eq(&repo.url))
            .one(db)
            .await?
        {
            let mut active: blacklist::ActiveModel = existing.into();
            active.reason = Set(reason.to_string());
            active.created_at = Set(chrono::Utc::now().naive_utc());
            let _ = active.update(db).await;
        } else {
            let blacklist_entry = blacklist::ActiveModel {
                repository_url: Set(repo.url.clone()),
                reason: Set(reason.to_string()),
                created_at: Set(chrono::Utc::now().naive_utc()),
                ..Default::default()
            };
            let _ = blacklist_entry.insert(db).await;
        }

        let mut active: skill_registry::ActiveModel = repo.clone().into();
        active.status = Set("blacklisted".to_string());
        active.blacklist_reason = Set(Some(reason.to_string()));
        active.blacklisted_at = Set(Some(chrono::Utc::now().naive_utc()));
        active.updated_at = Set(chrono::Utc::now().naive_utc());
        let _ = active.update(db).await?;

        Ok(())
    }
}

pub async fn fetch_pending_skills_activity(
    _ctx: ActContext,
    _input: (),
) -> Result<Vec<i32>, ActivityError> {
    let state = crate::get_app_state().await;
    SyncService::fetch_pending(&state.db)
        .await
        .map_err(ActivityError::from)
}

pub async fn sync_single_skill_activity(
    _ctx: ActContext,
    registry_id: i32,
) -> Result<SyncResult, ActivityError> {
    let state = crate::get_app_state().await;
    SyncService::process_one(&state.db, &state.s3, &state.github, registry_id)
        .await
        .map_err(ActivityError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{MockGithubApi, MockStorage};
    use migration::MigratorTrait;
    use sea_orm::{Database, EntityTrait, Set};
    use std::io::Write;
    use zip::write::FileOptions;

    fn create_zip(files: Vec<(&str, &[u8])>) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);
            for (path, content) in files {
                zip.start_file(path, options).unwrap();
                zip.write_all(content).unwrap();
            }
            zip.finish().unwrap();
        }
        buf
    }

    async fn setup_db() -> Result<DatabaseConnection> {
        let db = Database::connect("sqlite::memory:").await?;
        migration::Migrator::up(&db, None).await?;
        Ok(db)
    }

    async fn insert_registry(
        db: &DatabaseConnection,
        owner: &str,
        name: &str,
        url: &str,
    ) -> Result<i32> {
        let now = chrono::Utc::now().naive_utc();
        let active = skill_registry::ActiveModel {
            platform: Set(skill_registry::Platform::Github),
            owner: Set(owner.to_string()),
            name: Set(name.to_string()),
            url: Set(url.to_string()),
            description: Set(None),
            repo_type: Set(None),
            status: Set("active".to_string()),
            blacklist_reason: Set(None),
            blacklisted_at: Set(None),
            stars: Set(0),
            last_scanned_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };
        Ok(active.insert(db).await?.id)
    }

    #[tokio::test]
    async fn sync_skill_repo_marks_repo_type_and_creates_skill() -> Result<()> {
        let db = setup_db().await?;
        let registry_id = insert_registry(
            &db,
            "test-owner",
            "test-repo",
            "https://github.com/test-owner/test-repo",
        )
        .await?;

        let zip_data = create_zip(vec![(
            "skill-a/SKILL.md",
            b"---\nname: test-skill\ndescription: test description\nmetadata:\n  version: 1.0.0\n---\n# Body\n",
        )]);

        let mut github = MockGithubApi::new();
        github
            .expect_download_zipball()
            .returning(move |_, _| Ok(zip_data.clone()));

        let mut s3 = MockStorage::new();
        s3.expect_upload()
            .returning(|_, _| Ok("https://oss.example/test.zip".to_string()));

        let res = SyncService::process_one(&db, &s3, &github, registry_id).await?;
        assert!(matches!(res.status.as_str(), "Updated" | "Unchanged"));

        let repo = SkillRegistry::find_by_id(registry_id)
            .one(&db)
            .await?
            .unwrap();
        assert_eq!(repo.repo_type.as_deref(), Some("skill"));
        assert_eq!(repo.status, "active");

        let skills = Skills::find()
            .filter(skills::Column::SkillRegistryId.eq(registry_id))
            .all(&db)
            .await?;
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].is_active, 1);

        Ok(())
    }

    #[tokio::test]
    async fn sync_non_marketplace_without_valid_skill_blacklists_repo() -> Result<()> {
        let db = setup_db().await?;
        let registry_id = insert_registry(
            &db,
            "test-owner",
            "bad-repo",
            "https://github.com/test-owner/bad-repo",
        )
        .await?;

        let zip_data = create_zip(vec![("README.md", b"hello")]);

        let mut github = MockGithubApi::new();
        github
            .expect_download_zipball()
            .returning(move |_, _| Ok(zip_data.clone()));

        let mut s3 = MockStorage::new();
        s3.expect_upload()
            .returning(|_, _| Ok("https://oss.example/test.zip".to_string()));

        let res = SyncService::process_one(&db, &s3, &github, registry_id).await?;
        assert_eq!(res.status, "Blacklisted");

        let repo = SkillRegistry::find_by_id(registry_id)
            .one(&db)
            .await?
            .unwrap();
        assert_eq!(repo.status, "blacklisted");

        let blk = Blacklist::find()
            .filter(blacklist::Column::RepositoryUrl.eq("https://github.com/test-owner/bad-repo"))
            .one(&db)
            .await?;
        assert!(blk.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn sync_marketplace_repo_creates_plugins_versions_and_components() -> Result<()> {
        let db = setup_db().await?;
        let registry_id = insert_registry(
            &db,
            "test-owner",
            "market",
            "https://github.com/test-owner/market",
        )
        .await?;

        let marketplace_json = br#"{
  "name": "test-market",
  "plugins": [
    {
      "name": "p1",
      "description": "plugin one",
      "source": "./plugins/p1",
      "strict": true,
      "skills": ["./skills/s1"]
    }
  ]
}"#;

        let plugin_json = br#"{
  "name": "p1",
  "version": "1.2.3",
  "description": "plugin one"
}"#;

        let zip_data = create_zip(vec![
            (".claude-plugin/marketplace.json", marketplace_json),
            ("plugins/p1/.claude-plugin/plugin.json", plugin_json),
            (
                "plugins/p1/commands/hello.md",
                b"---\nname: hello\ndescription: hi\n---\n# cmd\n",
            ),
            (
                "plugins/p1/agents/reviewer.md",
                b"---\nname: reviewer\ndescription: reviews\n---\n# agent\n",
            ),
            (
                "plugins/p1/skills/s1/SKILL.md",
                b"---\nname: s1\ndescription: s1 desc\n---\n# skill\n",
            ),
        ]);

        let mut github = MockGithubApi::new();
        github
            .expect_download_zipball()
            .returning(move |_, _| Ok(zip_data.clone()));

        let mut s3 = MockStorage::new();
        s3.expect_upload()
            .returning(|_, _| Ok("https://oss.example/p1.zip".to_string()));

        let res = SyncService::process_one(&db, &s3, &github, registry_id).await?;
        assert!(matches!(res.status.as_str(), "Updated" | "Unchanged"));

        let repo = SkillRegistry::find_by_id(registry_id)
            .one(&db)
            .await?
            .unwrap();
        assert_eq!(repo.repo_type.as_deref(), Some("marketplace"));

        let plugins = Plugins::find()
            .filter(plugins::Column::SkillRegistryId.eq(registry_id))
            .all(&db)
            .await?;
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "p1");

        let versions = PluginVersions::find()
            .filter(plugin_versions::Column::PluginId.eq(plugins[0].id))
            .all(&db)
            .await?;
        assert_eq!(versions.len(), 1);

        let components = PluginComponents::find()
            .filter(plugin_components::Column::PluginVersionId.eq(versions[0].id))
            .all(&db)
            .await?;
        assert_eq!(components.len(), 3);

        Ok(())
    }
}
