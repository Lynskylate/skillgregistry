use crate::activities::{discovery::DiscoveryActivities, sync::SyncActivities};
use crate::contracts;
use crate::workflows;
use std::sync::Arc;
use temporalio_sdk::Worker;

pub fn register_activities(
    worker: &mut Worker,
    discovery: Arc<DiscoveryActivities>,
    sync: Arc<SyncActivities>,
) {
    let discovery_clone = Arc::clone(&discovery);
    worker.register_activity(contracts::activities::DISCOVERY, move |_ctx, queries| {
        let discovery = Arc::clone(&discovery_clone);
        async move { discovery.discover_repos(queries).await }
    });

    let discovery_clone = Arc::clone(&discovery);
    worker.register_activity(
        contracts::activities::FETCH_DUE_DISCOVERY_REGISTRIES,
        move |_ctx, _input: ()| {
            let discovery = Arc::clone(&discovery_clone);
            async move { discovery.fetch_due_registry_ids().await }
        },
    );

    let discovery_clone = Arc::clone(&discovery);
    worker.register_activity(
        contracts::activities::RUN_REGISTRY_DISCOVERY,
        move |_ctx, registry_id| {
            let discovery = Arc::clone(&discovery_clone);
            async move { discovery.run_registry_discovery(registry_id).await }
        },
    );

    let sync_clone = Arc::clone(&sync);
    worker.register_activity(
        contracts::activities::FETCH_PENDING_SKILLS,
        move |_ctx, input| {
            let sync = Arc::clone(&sync_clone);
            async move { sync.fetch_pending_skills(input).await }
        },
    );

    let sync_clone = Arc::clone(&sync);
    worker.register_activity(
        contracts::activities::SYNC_SINGLE_SKILL,
        move |_ctx, registry_id| {
            let sync = Arc::clone(&sync_clone);
            async move { sync.sync_single_skill(registry_id).await }
        },
    );

    let sync_clone = Arc::clone(&sync);
    worker.register_activity(
        contracts::activities::FETCH_REPO_SNAPSHOT,
        move |_ctx, registry_id| {
            let sync = Arc::clone(&sync_clone);
            async move { sync.fetch_repo_snapshot(registry_id).await }
        },
    );

    let sync_clone = Arc::clone(&sync);
    worker.register_activity(
        contracts::activities::APPLY_SYNC_FROM_SNAPSHOT,
        move |_ctx, snapshot| {
            let sync = Arc::clone(&sync_clone);
            async move { sync.apply_sync_from_snapshot(snapshot).await }
        },
    );
}

pub fn register_workflows(worker: &mut Worker) {
    worker.register_wf(
        contracts::workflows::DISCOVERY,
        workflows::discovery_workflow::discovery_workflow,
    );
    worker.register_wf(
        contracts::workflows::SYNC_SCHEDULER,
        workflows::sync_scheduler_workflow::sync_scheduler_workflow,
    );
    worker.register_wf(
        contracts::workflows::SYNC_REPO,
        workflows::sync_repo_workflow::sync_repo_workflow,
    );
    worker.register_wf(
        contracts::workflows::TRIGGER_REGISTRY,
        workflows::trigger_registry_workflow::trigger_registry_workflow,
    );
}
