pub mod activities {
    pub const DISCOVERY: &str = "discovery_activity";
    pub const FETCH_DUE_DISCOVERY_REGISTRIES: &str = "fetch_due_discovery_registries_activity";
    pub const RUN_REGISTRY_DISCOVERY: &str = "run_registry_discovery_activity";
    pub const FETCH_PENDING_SKILLS: &str = "fetch_pending_skills_activity";
    pub const SYNC_SINGLE_SKILL: &str = "sync_single_skill_activity";
    pub const FETCH_REPO_SNAPSHOT: &str = "fetch_repo_snapshot_activity";
    pub const APPLY_SYNC_FROM_SNAPSHOT: &str = "apply_sync_from_snapshot_activity";
}

pub mod workflows {
    pub const DISCOVERY: &str = "discovery_workflow";
    pub const SYNC_SCHEDULER: &str = "sync_scheduler_workflow";
    pub const SYNC_REPO: &str = "sync_repo_workflow";
    pub const TRIGGER_REGISTRY: &str = "trigger_registry_workflow";
}

pub const WORKFLOW_BATCH_CHUNK_SIZE: usize = 5;
