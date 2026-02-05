use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncResult {
    pub status: String,
    pub version: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepoSnapshotRef {
    pub registry_id: i32,
    pub owner: String,
    pub name: String,
    pub url: String,
    pub zip_hash: String,
    pub snapshot_s3_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind")]
pub enum SnapshotResult {
    Skipped { status: String },
    Snapshot(RepoSnapshotRef),
}

#[derive(Debug, Clone)]
pub struct SkillSyncOutcome {
    pub changed: bool,
    pub found_any: bool,
}

#[derive(Debug, Clone)]
pub struct PluginSyncOutcome {
    pub changed: bool,
    pub plugin_root_prefixes: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct NewPluginComponent {
    pub kind: String,
    pub path: String,
    pub name: String,
    pub description: Option<String>,
    pub markdown_content: String,
    pub metadata: Option<Value>,
}
