use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Deserialize, Debug)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    #[allow(dead_code)]
    pub license: Option<String>,
    #[allow(dead_code)]
    pub compatibility: Option<String>,
    #[serde(rename = "allowed-tools")]
    #[allow(dead_code)]
    pub allowed_tools: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncResult {
    pub status: String, // "Updated", "Unchanged", "Error", "SkippedBlacklisted", "Blacklisted"
    pub version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedMarkdown {
    pub metadata: Option<Value>,
    pub body: String,
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
