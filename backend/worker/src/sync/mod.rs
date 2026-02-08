pub mod domain;
pub mod download;
pub mod marketplace;
pub mod standalone;
pub mod utils;

use self::download::zip_to_file_map;
use self::marketplace::sync_marketplace_plugins;
use self::standalone::sync_standalone_skills;
use crate::ports::{GithubApi, Storage};
use anyhow::Result;
use common::domain::archive;
use common::entities::{
    blacklist,
    prelude::{Blacklist, SkillRegistry},
    skill_registry,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::Value;
use std::collections::HashSet;

pub use self::domain::{SnapshotResult, SyncResult};

pub struct SyncService {
    db: sea_orm::DatabaseConnection,
    s3: std::sync::Arc<dyn Storage>,
    github: std::sync::Arc<dyn GithubApi>,
    registry_service: std::sync::Arc<dyn common::services::registry::RegistryService>,
    discovery_registry_service:
        std::sync::Arc<dyn common::services::discovery_registries::DiscoveryRegistryService>,
}

impl SyncService {
    pub fn new(
        db: sea_orm::DatabaseConnection,
        s3: std::sync::Arc<dyn Storage>,
        github: std::sync::Arc<dyn GithubApi>,
        registry_service: std::sync::Arc<dyn common::services::registry::RegistryService>,
        discovery_registry_service: std::sync::Arc<
            dyn common::services::discovery_registries::DiscoveryRegistryService,
        >,
    ) -> Self {
        Self {
            db,
            s3,
            github,
            registry_service,
            discovery_registry_service,
        }
    }

    pub async fn fetch_pending(&self) -> Result<Vec<i32>> {
        let expiry_date = chrono::Utc::now().naive_utc() - chrono::Duration::days(30);
        self.registry_service
            .find_all_pending(expiry_date)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub async fn process_one(&self, registry_id: i32) -> Result<SyncResult> {
        tracing::debug!(registry_id, "process_one called");

        let repo = SkillRegistry::find()
            .filter(skill_registry::Column::Id.eq(registry_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Registry entry not found"))?;

        tracing::info!(
            "Processing repo: {}/{} (id={}, status='{}')",
            repo.owner,
            repo.name,
            repo.id,
            repo.status
        );

        if repo.status == "blacklisted" {
            return Ok(SyncResult {
                status: "SkippedBlacklisted".to_string(),
                version: None,
            });
        }

        let is_blacklisted = Blacklist::find()
            .filter(blacklist::Column::RepositoryUrl.eq(&repo.url))
            .one(&self.db)
            .await?
            .is_some();

        tracing::info!(
            "Blacklist check: url={}, is_blacklisted={}",
            repo.url,
            is_blacklisted
        );

        if is_blacklisted {
            tracing::info!("Returning SkippedBlacklisted due to blacklist entry");
            return Ok(SyncResult {
                status: "SkippedBlacklisted".to_string(),
                version: None,
            });
        }

        let token = self.resolve_repo_token(&repo).await?;
        let file_map = self
            .github
            .clone_repository_files(&repo.owner, &repo.name, &repo.url, token)
            .await?;

        let result = self.sync_from_file_map(&repo, &file_map).await;
        tracing::info!("sync_from_file_map result (from git clone): {:?}", result);
        result
    }

    pub async fn fetch_repo_snapshot(&self, registry_id: i32) -> Result<SnapshotResult> {
        let repo = SkillRegistry::find()
            .filter(skill_registry::Column::Id.eq(registry_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Registry entry not found"))?;

        if repo.status == "blacklisted" {
            return Ok(SnapshotResult::Skipped {
                status: "SkippedBlacklisted".to_string(),
            });
        }

        let is_blacklisted = Blacklist::find()
            .filter(blacklist::Column::RepositoryUrl.eq(&repo.url))
            .one(&self.db)
            .await?
            .is_some();

        if is_blacklisted {
            return Ok(SnapshotResult::Skipped {
                status: "SkippedBlacklisted".to_string(),
            });
        }

        let token = self.resolve_repo_token(&repo).await?;
        let file_map = self
            .github
            .clone_repository_files(&repo.owner, &repo.name, &repo.url, token)
            .await?;
        let zip_data = archive::package_zip(&file_map)?;
        let zip_hash = format!("{:x}", md5::compute(&zip_data));
        let snapshot_s3_key = format!("repo-snapshots/{}/{}.zip", repo.id, zip_hash);
        let _ = self.s3.upload(&snapshot_s3_key, zip_data).await?;

        Ok(SnapshotResult::Snapshot(domain::RepoSnapshotRef {
            registry_id: repo.id,
            owner: repo.owner,
            name: repo.name,
            url: repo.url,
            zip_hash,
            snapshot_s3_key,
        }))
    }

    async fn resolve_repo_token(&self, repo: &skill_registry::Model) -> Result<Option<String>> {
        let Some(discovery_registry_id) = repo.discovery_registry_id else {
            return Ok(None);
        };

        let cfg = self
            .discovery_registry_service
            .find_by_id(discovery_registry_id)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(cfg.map(|c| c.token))
    }

    pub async fn apply_sync_from_snapshot(
        &self,
        snapshot: &domain::RepoSnapshotRef,
    ) -> Result<SyncResult> {
        let zip_data = self.s3.download(&snapshot.snapshot_s3_key).await?;
        let file_map = match zip_to_file_map(&zip_data) {
            Ok(m) => m,
            Err(e) => {
                let repo = SkillRegistry::find()
                    .filter(skill_registry::Column::Id.eq(snapshot.registry_id))
                    .one(&self.db)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Registry entry not found"))?;
                let mut active: skill_registry::ActiveModel = repo.clone().into();
                active.status = Set("blacklisted".to_string());
                active.blacklist_reason = Set(Some(format!("Invalid zip archive: {}", e)));
                active.blacklisted_at = Set(Some(chrono::Utc::now().naive_utc()));
                active.update(&self.db).await?;
                return Ok(SyncResult {
                    status: "Blacklisted".to_string(),
                    version: None,
                });
            }
        };

        let repo = SkillRegistry::find()
            .filter(skill_registry::Column::Id.eq(snapshot.registry_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Registry entry not found"))?;

        self.sync_from_file_map(&repo, &file_map).await
    }

    async fn sync_from_file_map(
        &self,
        repo: &skill_registry::Model,
        file_map: &std::collections::BTreeMap<String, Vec<u8>>,
    ) -> Result<SyncResult> {
        let mut changed = false;

        if let Some(marketplace_bytes) = file_map.get(".claude-plugin/marketplace.json") {
            let marketplace_json: Value = match serde_json::from_slice(marketplace_bytes) {
                Ok(v) => v,
                Err(e) => {
                    let mut active: skill_registry::ActiveModel = repo.clone().into();
                    active.status = Set("blacklisted".to_string());
                    active.blacklist_reason = Set(Some(format!(
                        "Invalid marketplace.json (JSON parse error): {}",
                        e
                    )));
                    active.update(&self.db).await?;
                    return Ok(SyncResult {
                        status: "Blacklisted".to_string(),
                        version: None,
                    });
                }
            };

            let plugin_outcome =
                sync_marketplace_plugins(&self.db, &*self.s3, repo, file_map, &marketplace_json)
                    .await?;
            changed |= plugin_outcome.changed;

            let skill_outcome = sync_standalone_skills(
                &self.db,
                &*self.s3,
                repo,
                file_map,
                &plugin_outcome.plugin_root_prefixes,
                false,
            )
            .await?;
            changed |= skill_outcome.changed;

            let mut active: skill_registry::ActiveModel = repo.clone().into();
            active.status = Set("active".to_string());
            active.repo_type = Set(Some("marketplace".to_string()));
            active.update(&self.db).await?;
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
            sync_standalone_skills(&self.db, &*self.s3, repo, file_map, &HashSet::new(), true)
                .await?;
        changed |= skill_outcome.changed;

        if !skill_outcome.found_any {
            let mut active: skill_registry::ActiveModel = repo.clone().into();
            active.status = Set("blacklisted".to_string());
            active.blacklist_reason = Set(Some("No valid SKILL.md found".to_string()));
            active.update(&self.db).await?;
            return Ok(SyncResult {
                status: "Blacklisted".to_string(),
                version: None,
            });
        }

        tracing::debug!(repo_id = repo.id, "Updating skill registry entry to active");
        let mut active: skill_registry::ActiveModel = repo.clone().into();
        active.status = Set("active".to_string());
        active.repo_type = Set(Some("skill".to_string()));
        active.update(&self.db).await?;
        Ok(SyncResult {
            status: if changed {
                "Updated".to_string()
            } else {
                "Unchanged".to_string()
            },
            version: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{MockGithubApi, MockStorage};
    use common::build_all;
    use common::entities::{blacklist, prelude::*, skill_registry};
    use common::settings::Settings;
    use migration::MigratorTrait;
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter, Set,
    };
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::sync::Arc;
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

    fn create_file_map(entries: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
        entries
            .iter()
            .map(|(path, content)| ((*path).to_string(), content.as_bytes().to_vec()))
            .collect()
    }

    fn test_settings() -> Settings {
        Settings::new().unwrap_or_else(|_| Settings {
            port: 3000,
            database: common::settings::DatabaseSettings {
                url: "sqlite::memory:".to_string(),
            },
            s3: common::settings::S3Settings {
                bucket: "test".to_string(),
                region: "us-east-1".to_string(),
                endpoint: None,
                access_key_id: None,
                secret_access_key: None,
                force_path_style: false,
            },
            github: common::settings::GithubSettings {
                search_keywords: "topic:agent-skill".to_string(),
                token: None,
                api_url: "https://api.github.com".to_string(),
            },
            worker: common::settings::WorkerSettings {
                scan_interval_seconds: 3600,
            },
            temporal: common::settings::TemporalSettings {
                server_url: "http://localhost:7233".to_string(),
                task_queue: "test".to_string(),
            },
            auth: common::settings::AuthSettings::default(),
            debug: true,
        })
    }

    async fn setup_db_and_services() -> anyhow::Result<(DatabaseConnection, common::Services)> {
        let db = Database::connect("sqlite::memory:").await?;
        migration::Migrator::up(&db, None).await?;

        let settings = test_settings();
        let db_arc = Arc::new(db.clone());
        let (_repos, services) = build_all(db_arc, &settings).await?;
        Ok((db, services))
    }

    async fn insert_registry(
        db: &DatabaseConnection,
        owner: &str,
        name: &str,
        status: &str,
        url: &str,
    ) -> skill_registry::Model {
        skill_registry::ActiveModel {
            platform: Set(skill_registry::Platform::Github),
            owner: Set(owner.to_string()),
            name: Set(name.to_string()),
            url: Set(url.to_string()),
            status: Set(status.to_string()),
            stars: Set(0),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn fetch_pending_unblacklists_expired_blacklist_entries() -> Result<()> {
        let (db, services) = setup_db_and_services().await?;

        let expiry_date = chrono::Utc::now().naive_utc() - chrono::Duration::days(30);
        let old_date = expiry_date - chrono::Duration::days(1);

        insert_registry(
            &db,
            "test-owner",
            "test-repo",
            "active",
            "https://github.com/test/test",
        )
        .await;

        let blacklist_entry = blacklist::ActiveModel {
            repository_url: Set("https://github.com/test/test".to_string()),
            reason: Set("old reason".to_string()),
            created_at: Set(old_date),
            ..Default::default()
        };
        blacklist_entry.insert(&db).await?;

        let sync_service = SyncService::new(
            db.clone(),
            Arc::new(MockStorage::new()),
            Arc::new(MockGithubApi::new()),
            services.registry_service,
            services.discovery_registry_service,
        );

        let pending = sync_service.fetch_pending().await?;
        assert_eq!(pending, vec![1]);

        let updated_repo = SkillRegistry::find_by_id(1).one(&db).await?.unwrap();
        assert_eq!(updated_repo.status, "active");
        assert!(updated_repo.blacklist_reason.is_none());
        assert!(updated_repo.blacklisted_at.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn process_one_errors_for_missing_registry() -> Result<()> {
        let (db, services) = setup_db_and_services().await?;
        let sync_service = SyncService::new(
            db,
            Arc::new(MockStorage::new()),
            Arc::new(MockGithubApi::new()),
            services.registry_service,
            services.discovery_registry_service,
        );

        let err = sync_service.process_one(404).await.unwrap_err();
        assert!(err.to_string().contains("Registry entry not found"));
        Ok(())
    }

    #[tokio::test]
    async fn process_one_skips_blacklisted_registry_status() -> Result<()> {
        let (db, services) = setup_db_and_services().await?;
        let repo = insert_registry(
            &db,
            "acme",
            "blacklisted-repo",
            "blacklisted",
            "https://github.com/acme/blacklisted-repo",
        )
        .await;

        let sync_service = SyncService::new(
            db,
            Arc::new(MockStorage::new()),
            Arc::new(MockGithubApi::new()),
            services.registry_service,
            services.discovery_registry_service,
        );

        let result = sync_service.process_one(repo.id).await?;
        assert_eq!(result.status, "SkippedBlacklisted");
        Ok(())
    }

    #[tokio::test]
    async fn process_one_blacklists_repo_without_valid_skill() -> Result<()> {
        let (db, services) = setup_db_and_services().await?;
        let repo = insert_registry(
            &db,
            "acme",
            "empty-repo",
            "active",
            "https://github.com/acme/empty-repo",
        )
        .await;

        let mut github = MockGithubApi::new();
        github
            .expect_clone_repository_files()
            .times(1)
            .returning(|owner, repo, url, token| {
                assert_eq!(owner, "acme");
                assert_eq!(repo, "empty-repo");
                assert_eq!(url, "https://github.com/acme/empty-repo");
                assert!(token.is_none());
                Ok(BTreeMap::new())
            });

        let sync_service = SyncService::new(
            db.clone(),
            Arc::new(MockStorage::new()),
            Arc::new(github),
            services.registry_service,
            services.discovery_registry_service,
        );

        let result = sync_service.process_one(repo.id).await?;
        assert_eq!(result.status, "Blacklisted");

        let updated = SkillRegistry::find_by_id(repo.id).one(&db).await?.unwrap();
        assert_eq!(updated.status, "blacklisted");
        assert_eq!(
            updated.blacklist_reason.as_deref(),
            Some("No valid SKILL.md found")
        );
        Ok(())
    }

    #[tokio::test]
    async fn fetch_repo_snapshot_uploads_snapshot_archive() -> Result<()> {
        let (db, services) = setup_db_and_services().await?;
        let repo = insert_registry(
            &db,
            "acme",
            "snapshot-repo",
            "active",
            "https://github.com/acme/snapshot-repo",
        )
        .await;

        let files = create_file_map(&[(
            "demo/SKILL.md",
            "---
name: demo-skill
description: demo
---
# Demo
",
        )]);

        let mut github = MockGithubApi::new();
        github
            .expect_clone_repository_files()
            .times(1)
            .returning(move |_, _, _, _| Ok(files.clone()));

        let mut storage = MockStorage::new();
        storage.expect_upload().times(1).returning(|key, body| {
            assert!(key.starts_with("repo-snapshots/"));
            assert!(key.ends_with(".zip"));
            assert!(!body.is_empty());
            Ok(format!("https://oss.local/{key}"))
        });

        let sync_service = SyncService::new(
            db,
            Arc::new(storage),
            Arc::new(github),
            services.registry_service,
            services.discovery_registry_service,
        );

        let snapshot = sync_service.fetch_repo_snapshot(repo.id).await?;
        match snapshot {
            SnapshotResult::Snapshot(snapshot_ref) => {
                assert_eq!(snapshot_ref.registry_id, repo.id);
                assert!(snapshot_ref.snapshot_s3_key.starts_with("repo-snapshots/"));
            }
            SnapshotResult::Skipped { .. } => panic!("expected snapshot output"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn apply_sync_from_snapshot_blacklists_invalid_zip() -> Result<()> {
        let (db, services) = setup_db_and_services().await?;
        let repo = insert_registry(
            &db,
            "acme",
            "invalid-zip-repo",
            "active",
            "https://github.com/acme/invalid-zip-repo",
        )
        .await;

        let mut storage = MockStorage::new();
        storage
            .expect_download()
            .times(1)
            .returning(|_| Ok(vec![0, 1, 2, 3]));

        let sync_service = SyncService::new(
            db.clone(),
            Arc::new(storage),
            Arc::new(MockGithubApi::new()),
            services.registry_service,
            services.discovery_registry_service,
        );

        let snapshot = domain::RepoSnapshotRef {
            registry_id: repo.id,
            owner: repo.owner.clone(),
            name: repo.name.clone(),
            url: repo.url.clone(),
            zip_hash: "deadbeef".to_string(),
            snapshot_s3_key: "repo-snapshots/invalid.zip".to_string(),
        };

        let result = sync_service.apply_sync_from_snapshot(&snapshot).await?;
        assert_eq!(result.status, "Blacklisted");

        let updated = SkillRegistry::find_by_id(repo.id).one(&db).await?.unwrap();
        assert_eq!(updated.status, "blacklisted");
        assert!(updated
            .blacklist_reason
            .as_deref()
            .unwrap_or_default()
            .contains("Invalid zip archive"));

        Ok(())
    }

    #[tokio::test]
    async fn apply_sync_from_snapshot_processes_valid_archive() -> Result<()> {
        let (db, services) = setup_db_and_services().await?;
        let repo = insert_registry(
            &db,
            "acme",
            "valid-zip-repo",
            "active",
            "https://github.com/acme/valid-zip-repo",
        )
        .await;

        let zip_bytes = create_zip(vec![(
            "skill/SKILL.md",
            b"---
name: zip-skill
description: zip skill
metadata:
  version: 1.0.0
---
# Body
",
        )]);

        let mut storage = MockStorage::new();
        storage
            .expect_download()
            .times(1)
            .returning(move |_| Ok(zip_bytes.clone()));
        storage
            .expect_upload()
            .times(1)
            .returning(|key, _| Ok(format!("https://oss.local/{key}")));

        let sync_service = SyncService::new(
            db.clone(),
            Arc::new(storage),
            Arc::new(MockGithubApi::new()),
            services.registry_service,
            services.discovery_registry_service,
        );

        let snapshot = domain::RepoSnapshotRef {
            registry_id: repo.id,
            owner: repo.owner.clone(),
            name: repo.name.clone(),
            url: repo.url.clone(),
            zip_hash: "hash".to_string(),
            snapshot_s3_key: "repo-snapshots/valid.zip".to_string(),
        };

        let result = sync_service.apply_sync_from_snapshot(&snapshot).await?;
        assert_eq!(result.status, "Updated");

        let updated = SkillRegistry::find_by_id(repo.id).one(&db).await?.unwrap();
        assert_eq!(updated.status, "active");
        assert_eq!(updated.repo_type.as_deref(), Some("skill"));

        let skills = Skills::find()
            .filter(common::entities::skills::Column::SkillRegistryId.eq(repo.id))
            .all(&db)
            .await?;
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "zip-skill");

        Ok(())
    }
}
