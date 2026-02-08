pub use crate::sync::domain::{RepoSnapshotRef, SnapshotResult, SyncResult};
use std::sync::Arc;
use temporalio_sdk::ActivityError;

pub struct SyncActivities {
    sync_service: Arc<crate::sync::SyncService>,
}

impl SyncActivities {
    pub fn new(sync_service: Arc<crate::sync::SyncService>) -> Self {
        Self { sync_service }
    }

    pub async fn fetch_pending_skills(&self, _input: ()) -> Result<Vec<i32>, ActivityError> {
        self.sync_service
            .fetch_pending()
            .await
            .map_err(ActivityError::from)
    }

    pub async fn sync_single_skill(&self, registry_id: i32) -> Result<SyncResult, ActivityError> {
        self.sync_service
            .process_one(registry_id)
            .await
            .map_err(ActivityError::from)
    }

    pub async fn fetch_repo_snapshot(
        &self,
        registry_id: i32,
    ) -> Result<SnapshotResult, ActivityError> {
        self.sync_service
            .fetch_repo_snapshot(registry_id)
            .await
            .map_err(ActivityError::from)
    }

    pub async fn apply_sync_from_snapshot(
        &self,
        snapshot: RepoSnapshotRef,
    ) -> Result<SyncResult, ActivityError> {
        self.sync_service
            .apply_sync_from_snapshot(&snapshot)
            .await
            .map_err(ActivityError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{MockGithubApi, MockStorage};
    use common::build_all;
    use common::entities::skill_registry;
    use common::settings::Settings;
    use migration::MigratorTrait;
    use sea_orm::{ActiveModelTrait, Database, Set};
    use std::collections::BTreeMap;
    use std::sync::Arc;

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

    #[tokio::test]
    async fn sync_activities_forward_to_sync_service() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        migration::Migrator::up(&db, None).await.unwrap();

        let settings = test_settings();
        let db_arc = Arc::new(db.clone());
        let (_repos, services) = build_all(db_arc, &settings).await.unwrap();

        let registry = skill_registry::ActiveModel {
            platform: Set(skill_registry::Platform::Github),
            owner: Set("acme".to_string()),
            name: Set("activities-repo".to_string()),
            url: Set("https://github.com/acme/activities-repo".to_string()),
            status: Set("active".to_string()),
            stars: Set(0),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();

        let file_map = BTreeMap::from([(
            "skill/SKILL.md".to_string(),
            b"---\nname: activity-skill\ndescription: activity\n---\n# Body\n".to_vec(),
        )]);
        let snapshot_zip = common::domain::archive::package_zip(&file_map).unwrap();

        let mut github = MockGithubApi::new();
        github
            .expect_clone_repository_files()
            .times(2)
            .returning(move |_, _, _, _| Ok(file_map.clone()));

        let mut storage = MockStorage::new();
        storage
            .expect_upload()
            .times(2)
            .returning(|key, _| Ok(format!("https://oss.local/{key}")));
        storage
            .expect_download()
            .times(1)
            .returning(move |_| Ok(snapshot_zip.clone()));

        let sync_service = Arc::new(crate::sync::SyncService::new(
            db.clone(),
            Arc::new(storage),
            Arc::new(github),
            services.registry_service,
            services.discovery_registry_service,
        ));

        let activities = SyncActivities::new(sync_service);

        let pending = activities.fetch_pending_skills(()).await.unwrap();
        assert_eq!(pending, vec![registry.id]);

        let sync_result = activities.sync_single_skill(registry.id).await.unwrap();
        assert_eq!(sync_result.status, "Updated");

        let snapshot = activities.fetch_repo_snapshot(registry.id).await.unwrap();
        let snapshot_ref = match snapshot {
            SnapshotResult::Snapshot(value) => value,
            SnapshotResult::Skipped { .. } => panic!("expected snapshot result"),
        };
        assert_eq!(snapshot_ref.registry_id, registry.id);

        let apply_result = activities
            .apply_sync_from_snapshot(snapshot_ref)
            .await
            .unwrap();
        assert!(matches!(
            apply_result.status.as_str(),
            "Updated" | "Unchanged"
        ));
    }
}
