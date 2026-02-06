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
