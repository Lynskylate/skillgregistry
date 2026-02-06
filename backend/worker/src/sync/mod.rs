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
}

impl SyncService {
    pub fn new(
        db: sea_orm::DatabaseConnection,
        s3: std::sync::Arc<dyn Storage>,
        github: std::sync::Arc<dyn GithubApi>,
        registry_service: std::sync::Arc<dyn common::services::registry::RegistryService>,
    ) -> Self {
        Self {
            db,
            s3,
            github,
            registry_service,
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

        let zip_data = self
            .github
            .download_zipball(&repo.owner, &repo.name)
            .await?;
        tracing::info!("Downloaded zip: {} bytes", zip_data.len());

        let file_map = match zip_to_file_map(&zip_data) {
            Ok(m) => m,
            Err(e) => {
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

        let result = self.sync_from_file_map(&repo, &file_map).await;
        tracing::info!("sync_from_file_map result (from zip): {:?}", result);
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

        let zip_data = self
            .github
            .download_zipball(&repo.owner, &repo.name)
            .await?;
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
    use common::build_all;
    use common::entities::{blacklist, skill_registry};
    use common::settings::Settings;
    use migration::MigratorTrait;
    use sea_orm::{Database, Set};
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

    #[tokio::test]
    async fn fetch_pending_unblacklists_expired_blacklist_entries() -> Result<()> {
        let db = Database::connect("sqlite::memory:").await?;
        migration::Migrator::up(&db, None).await?;

        let settings = Settings::new().unwrap_or_else(|_| Settings {
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
        });

        let db_arc = std::sync::Arc::new(db.clone());
        let (_repos, services) = build_all(db_arc, &settings).await;

        let expiry_date = chrono::Utc::now().naive_utc() - chrono::Duration::days(30);
        let old_date = expiry_date - chrono::Duration::days(1);

        let repo = skill_registry::ActiveModel {
            platform: Set(skill_registry::Platform::Github),
            owner: Set("test-owner".to_string()),
            name: Set("test-repo".to_string()),
            url: Set("https://github.com/test/test".to_string()),
            status: Set("active".to_string()),
            stars: Set(0),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        };
        repo.insert(&db).await?;

        let blacklist_entry = blacklist::ActiveModel {
            repository_url: Set("https://github.com/test/test".to_string()),
            reason: Set("old reason".to_string()),
            created_at: Set(old_date),
            ..Default::default()
        };
        blacklist_entry.insert(&db).await?;

        let sync_service = SyncService::new(
            db.clone(),
            std::sync::Arc::new(crate::ports::MockStorage::new()),
            std::sync::Arc::new(crate::ports::MockGithubApi::new()),
            services.registry_service,
        );

        let pending = sync_service.fetch_pending().await?;
        assert!(
            !pending.is_empty(),
            "Repo should be in pending since blacklist expired"
        );
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0], 1);

        let updated_repo = SkillRegistry::find_by_id(1).one(&db).await?.unwrap();
        assert_eq!(updated_repo.status, "active");
        assert!(updated_repo.blacklist_reason.is_none());
        assert!(updated_repo.blacklisted_at.is_none());

        Ok(())
    }
}
