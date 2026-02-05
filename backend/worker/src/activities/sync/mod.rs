pub use crate::sync::domain::{RepoSnapshotRef, SnapshotResult, SyncResult};
use temporalio_sdk::ActivityError;

pub async fn fetch_pending_skills_activity_with_ctx(
    ctx: &crate::WorkerContext,
    _input: (),
) -> Result<Vec<i32>, ActivityError> {
    crate::sync::SyncService::fetch_pending(ctx.services.registry_service.as_ref())
        .await
        .map_err(ActivityError::from)
}

pub async fn sync_single_skill_activity_with_ctx(
    ctx: &crate::WorkerContext,
    registry_id: i32,
) -> Result<SyncResult, ActivityError> {
    crate::sync::SyncService::process_one(
        ctx.db.as_ref(),
        ctx.services.s3.as_ref(),
        ctx.github.as_ref(),
        registry_id,
    )
    .await
    .map_err(ActivityError::from)
}

pub async fn fetch_repo_snapshot_activity_with_ctx(
    ctx: &crate::WorkerContext,
    registry_id: i32,
) -> Result<SnapshotResult, ActivityError> {
    crate::sync::SyncService::fetch_repo_snapshot(
        ctx.db.as_ref(),
        ctx.services.s3.as_ref(),
        ctx.github.as_ref(),
        registry_id,
    )
    .await
    .map_err(ActivityError::from)
}

pub async fn apply_sync_from_snapshot_activity_with_ctx(
    ctx: &crate::WorkerContext,
    snapshot: RepoSnapshotRef,
) -> Result<SyncResult, ActivityError> {
    crate::sync::SyncService::apply_sync_from_snapshot(
        ctx.db.as_ref(),
        ctx.services.s3.as_ref(),
        &snapshot,
    )
    .await
    .map_err(ActivityError::from)
}
