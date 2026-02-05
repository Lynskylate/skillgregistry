use super::domain::{NewPluginComponent, PluginSyncOutcome};
use super::utils::{
    compute_hash, json_string, normalize_dir_prefix, package_skill, parse_boolish,
    parse_markdown_frontmatter, subtree_file_map,
};
use crate::ports::Storage;
use anyhow::Result;
use common::entities::{plugin_components, plugin_versions, plugins, prelude::*, skill_registry};
use sea_orm::*;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

pub async fn sync_marketplace_plugins(
    db: &sea_orm::DatabaseConnection,
    s3: &impl Storage,
    repo: &skill_registry::Model,
    all_files: &BTreeMap<String, Vec<u8>>,
    marketplace: &Value,
) -> Result<PluginSyncOutcome> {
    let plugin_entries = marketplace
        .get("plugins")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut changed = false;
    let mut seen_plugin_names = HashSet::new();
    let mut plugin_root_prefixes = HashSet::new();

    for entry in plugin_entries {
        let plugin_name = match entry.get("name").and_then(|v| v.as_str()) {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => continue,
        };
        seen_plugin_names.insert(plugin_name.clone());

        let description = json_string(entry.get("description"));
        let strict = parse_boolish(entry.get("strict"));
        let source_value = entry
            .get("source")
            .cloned()
            .unwrap_or(Value::String("./".to_string()));

        let plugin = upsert_plugin(
            db,
            repo.id,
            &plugin_name,
            description.clone(),
            source_value.clone(),
            strict,
        )
        .await?;

        let source_str = match source_value.as_str() {
            Some(s) => s.to_string(),
            None => {
                continue;
            }
        };
        let plugin_root = source_str
            .trim()
            .trim_start_matches("./")
            .trim_start_matches('/')
            .trim_end_matches('/')
            .to_string();

        let manifest_path = if plugin_root.is_empty() {
            ".claude-plugin/plugin.json".to_string()
        } else {
            format!("{}/.claude-plugin/plugin.json", plugin_root)
        };
        let manifest_json = all_files
            .get(&manifest_path)
            .and_then(|b| serde_json::from_slice::<Value>(b).ok());
        let manifest = manifest_json.clone().unwrap_or_else(|| entry.clone());

        let manifest_dirs = extract_component_dirs_from_manifest(&manifest);
        let commands_dir = manifest_dirs
            .get("commands")
            .cloned()
            .unwrap_or_else(|| "commands".to_string());
        let agents_dir = manifest_dirs
            .get("agents")
            .cloned()
            .unwrap_or_else(|| "agents".to_string());
        let skills_dir = manifest_dirs
            .get("skills")
            .cloned()
            .unwrap_or_else(|| "skills".to_string());

        plugin_root_prefixes.insert(join_plugin_path(&plugin_root, &commands_dir));
        plugin_root_prefixes.insert(join_plugin_path(&plugin_root, &agents_dir));
        plugin_root_prefixes.insert(join_plugin_path(&plugin_root, &skills_dir));

        if let Some(arr) = entry.get("skills").and_then(|v| v.as_array()) {
            for v in arr {
                if let Some(s) = v.as_str() {
                    plugin_root_prefixes.insert(join_plugin_path(&plugin_root, s));
                }
            }
        }

        let plugin_files = subtree_file_map(all_files, &plugin_root);
        let hash_string = compute_hash(&plugin_files);
        let derived_patch = u32::from_str_radix(&hash_string[..8], 16).unwrap_or(0);
        let version_str = json_string(manifest.get("version"))
            .or_else(|| json_string(entry.get("version")))
            .unwrap_or_else(|| format!("0.0.{}", derived_patch));

        let existing_version = PluginVersions::find()
            .filter(plugin_versions::Column::PluginId.eq(plugin.id))
            .filter(plugin_versions::Column::Version.eq(&version_str))
            .one(db)
            .await?;

        let unchanged = existing_version
            .as_ref()
            .and_then(|v| v.file_hash.as_ref())
            .map(|h| h == &hash_string)
            .unwrap_or(false);

        if unchanged {
            continue;
        }

        let new_zip_buffer = package_skill(&plugin_files)?;
        let s3_key = format!("plugins/{}/{}.zip", plugin_name, version_str);
        let oss_url = s3.upload(&s3_key, new_zip_buffer).await?;

        let metadata = serde_json::json!({
            "marketplace_entry": entry,
            "manifest": manifest_json.unwrap_or(Value::Null),
            "resolved_manifest": manifest,
            "source": source_value,
            "strict": strict,
        });

        let txn = db.begin().await?;
        let plugin_version_id = if let Some(v) = existing_version {
            let mut active: plugin_versions::ActiveModel = v.into();
            active.description = Set(description.clone());
            active.readme_content = Set(None);
            active.s3_key = Set(Some(s3_key));
            active.oss_url = Set(Some(oss_url));
            active.file_hash = Set(Some(hash_string));
            active.metadata = Set(Some(metadata));
            active.update(&txn).await?.id
        } else {
            let new_version = plugin_versions::ActiveModel {
                plugin_id: Set(plugin.id),
                version: Set(version_str.clone()),
                description: Set(description.clone()),
                readme_content: Set(None),
                s3_key: Set(Some(s3_key)),
                oss_url: Set(Some(oss_url)),
                file_hash: Set(Some(hash_string)),
                metadata: Set(Some(metadata)),
                created_at: Set(chrono::Utc::now().naive_utc()),
                ..Default::default()
            };
            new_version.insert(&txn).await?.id
        };

        let components =
            collect_plugin_components(all_files, &plugin_root, &entry, &manifest, strict)?;

        let _ = PluginComponents::delete_many()
            .filter(plugin_components::Column::PluginVersionId.eq(plugin_version_id))
            .exec(&txn)
            .await?;

        if !components.is_empty() {
            let active_models: Vec<plugin_components::ActiveModel> = components
                .into_iter()
                .map(|c| plugin_components::ActiveModel {
                    plugin_version_id: Set(plugin_version_id),
                    kind: Set(c.kind),
                    path: Set(c.path),
                    name: Set(c.name),
                    description: Set(c.description),
                    markdown_content: Set(Some(c.markdown_content)),
                    metadata: Set(c.metadata),
                    created_at: Set(chrono::Utc::now().naive_utc()),
                    ..Default::default()
                })
                .collect();
            let _ = PluginComponents::insert_many(active_models)
                .exec(&txn)
                .await?;
        }

        let mut plugin_active: plugins::ActiveModel = plugin.into();
        plugin_active.latest_version = Set(Some(version_str));
        plugin_active.updated_at = Set(chrono::Utc::now().naive_utc());
        let _ = plugin_active.update(&txn).await?;
        txn.commit().await?;

        changed = true;
    }

    let existing_plugins = Plugins::find()
        .filter(plugins::Column::SkillRegistryId.eq(repo.id))
        .all(db)
        .await?;
    for p in existing_plugins {
        if !seen_plugin_names.contains(&p.name) && p.is_active != 0 {
            let mut active: plugins::ActiveModel = p.into();
            active.is_active = Set(0);
            active.updated_at = Set(chrono::Utc::now().naive_utc());
            let _ = active.update(db).await?;
        }
    }

    Ok(PluginSyncOutcome {
        changed,
        plugin_root_prefixes,
    })
}

async fn upsert_plugin(
    db: &DatabaseConnection,
    registry_id: i32,
    name: &str,
    description: Option<String>,
    source: Value,
    strict: bool,
) -> Result<plugins::Model> {
    if let Some(existing) = Plugins::find()
        .filter(plugins::Column::SkillRegistryId.eq(registry_id))
        .filter(plugins::Column::Name.eq(name))
        .one(db)
        .await?
    {
        let mut active: plugins::ActiveModel = existing.into();
        active.description = Set(description);
        active.source = Set(Some(source));
        active.strict = Set(if strict { 1 } else { 0 });
        active.is_active = Set(1);
        active.updated_at = Set(chrono::Utc::now().naive_utc());
        Ok(active.update(db).await?)
    } else {
        let active = plugins::ActiveModel {
            skill_registry_id: Set(registry_id),
            name: Set(name.to_string()),
            description: Set(description),
            source: Set(Some(source)),
            strict: Set(if strict { 1 } else { 0 }),
            latest_version: Set(None),
            is_active: Set(1),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        };
        Ok(active.insert(db).await?)
    }
}

fn collect_plugin_components(
    all_files: &BTreeMap<String, Vec<u8>>,
    plugin_root: &str,
    marketplace_entry: &Value,
    manifest: &Value,
    strict: bool,
) -> Result<Vec<NewPluginComponent>> {
    let root_prefix = normalize_dir_prefix(plugin_root);

    let manifest_dirs = extract_component_dirs_from_manifest(manifest);
    let commands_dir = manifest_dirs
        .get("commands")
        .cloned()
        .unwrap_or_else(|| "commands".to_string());
    let agents_dir = manifest_dirs
        .get("agents")
        .cloned()
        .unwrap_or_else(|| "agents".to_string());
    let skills_dir = manifest_dirs
        .get("skills")
        .cloned()
        .unwrap_or_else(|| "skills".to_string());

    let mut out = Vec::new();
    let mut seen = HashSet::new();

    let explicit_commands = marketplace_entry.get("commands").and_then(|v| v.as_array());
    let explicit_agents = marketplace_entry.get("agents").and_then(|v| v.as_array());
    let explicit_skills = marketplace_entry.get("skills").and_then(|v| v.as_array());

    let mut command_candidates = Vec::new();
    if let Some(arr) = explicit_commands {
        command_candidates.extend(expand_explicit_md_paths(all_files, plugin_root, arr, false));
    }
    if !strict || explicit_commands.is_none() {
        command_candidates.extend(scan_command_files(all_files, &root_prefix, &commands_dir));
    }

    for p in command_candidates {
        if !seen.insert(format!("command:{}", p)) {
            continue;
        }
        if let Some(comp) = parse_component_file(all_files, plugin_root, &p, "command")? {
            out.push(comp);
        }
    }

    let mut agent_candidates = Vec::new();
    if let Some(arr) = explicit_agents {
        agent_candidates.extend(expand_explicit_md_paths(all_files, plugin_root, arr, false));
    }
    if !strict || explicit_agents.is_none() {
        agent_candidates.extend(scan_md_files(all_files, &root_prefix, &agents_dir));
    }

    for p in agent_candidates {
        if !seen.insert(format!("agent:{}", p)) {
            continue;
        }
        if let Some(comp) = parse_component_file(all_files, plugin_root, &p, "agent")? {
            out.push(comp);
        }
    }

    let mut skill_candidates = Vec::new();
    if let Some(arr) = explicit_skills {
        skill_candidates.extend(resolve_explicit_skill_paths(arr, plugin_root));
    }
    if !strict || explicit_skills.is_none() {
        skill_candidates.extend(scan_skill_files(all_files, &root_prefix, &skills_dir));
        skill_candidates.extend(scan_skill_files(all_files, &root_prefix, &commands_dir));
    }

    for p in skill_candidates {
        if !seen.insert(format!("skill:{}", p)) {
            continue;
        }
        if let Some(comp) = parse_component_file(all_files, plugin_root, &p, "skill")? {
            out.push(comp);
        }
    }

    Ok(out)
}

fn extract_component_dirs_from_manifest(manifest: &Value) -> HashMap<String, String> {
    let mut out = HashMap::new();

    let commands = json_string(manifest.get("commandsDir"))
        .or_else(|| json_string(manifest.get("commands_dir")))
        .or_else(|| {
            manifest
                .get("commands")
                .and_then(|v| json_string(v.get("path")))
        })
        .or_else(|| {
            manifest
                .get("commands")
                .and_then(|v| json_string(v.get("dir")))
        })
        .or_else(|| {
            manifest
                .get("commands")
                .and_then(|v| json_string(v.get("directory")))
        });
    if let Some(p) = commands {
        out.insert("commands".to_string(), p);
    }

    let agents = json_string(manifest.get("agentsDir"))
        .or_else(|| json_string(manifest.get("agents_dir")))
        .or_else(|| {
            manifest
                .get("agents")
                .and_then(|v| json_string(v.get("path")))
        })
        .or_else(|| {
            manifest
                .get("agents")
                .and_then(|v| json_string(v.get("dir")))
        })
        .or_else(|| {
            manifest
                .get("agents")
                .and_then(|v| json_string(v.get("directory")))
        });
    if let Some(p) = agents {
        out.insert("agents".to_string(), p);
    }

    let skills = json_string(manifest.get("skillsDir"))
        .or_else(|| json_string(manifest.get("skills_dir")))
        .or_else(|| {
            manifest
                .get("skills")
                .and_then(|v| json_string(v.get("path")))
        })
        .or_else(|| {
            manifest
                .get("skills")
                .and_then(|v| json_string(v.get("dir")))
        })
        .or_else(|| {
            manifest
                .get("skills")
                .and_then(|v| json_string(v.get("directory")))
        });
    if let Some(p) = skills {
        out.insert("skills".to_string(), p);
    }

    out
}

fn join_plugin_path(plugin_root: &str, rel: &str) -> String {
    let rel = rel
        .trim()
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    let plugin_root = plugin_root
        .trim()
        .trim_start_matches("./")
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string();
    if plugin_root.is_empty() {
        rel
    } else if rel.is_empty() {
        plugin_root
    } else {
        format!("{}/{}", plugin_root, rel)
    }
}

fn expand_explicit_md_paths(
    all_files: &BTreeMap<String, Vec<u8>>,
    plugin_root: &str,
    arr: &[Value],
    allow_skill_md: bool,
) -> Vec<String> {
    let mut out = Vec::new();
    for v in arr {
        let s = match v.as_str() {
            Some(s) => s.trim(),
            None => continue,
        };
        if s.is_empty() {
            continue;
        }

        let p = join_plugin_path(plugin_root, s);
        if p.ends_with(".md") {
            if allow_skill_md || !p.ends_with("SKILL.md") {
                out.push(p);
            }
            continue;
        }

        let prefix = normalize_dir_prefix(&p);
        for path in all_files.keys() {
            if path.starts_with(&prefix)
                && path.ends_with(".md")
                && (allow_skill_md || !path.ends_with("SKILL.md"))
            {
                out.push(path.clone());
            }
        }
    }
    out
}

fn resolve_explicit_skill_paths(arr: &[Value], plugin_root: &str) -> Vec<String> {
    let mut out = Vec::new();
    for v in arr {
        if let Some(s) = v.as_str() {
            let s = s.trim();
            if s.is_empty() {
                continue;
            }
            let p = join_plugin_path(plugin_root, s);
            if p.ends_with(".md") {
                out.push(p);
            } else {
                out.push(format!("{}/SKILL.md", p.trim_end_matches('/')));
            }
        }
    }
    out
}

fn scan_md_files(
    all_files: &BTreeMap<String, Vec<u8>>,
    root_prefix: &str,
    dir: &str,
) -> Vec<String> {
    let prefix = if root_prefix.is_empty() {
        normalize_dir_prefix(dir)
    } else {
        format!("{}{}", root_prefix, normalize_dir_prefix(dir))
    };

    all_files
        .keys()
        .filter(|p| p.starts_with(&prefix) && p.ends_with(".md"))
        .cloned()
        .collect()
}

fn scan_command_files(
    all_files: &BTreeMap<String, Vec<u8>>,
    root_prefix: &str,
    dir: &str,
) -> Vec<String> {
    scan_md_files(all_files, root_prefix, dir)
        .into_iter()
        .filter(|p| !p.ends_with("SKILL.md"))
        .collect()
}

fn scan_skill_files(
    all_files: &BTreeMap<String, Vec<u8>>,
    root_prefix: &str,
    dir: &str,
) -> Vec<String> {
    let prefix = if root_prefix.is_empty() {
        normalize_dir_prefix(dir)
    } else {
        format!("{}{}", root_prefix, normalize_dir_prefix(dir))
    };
    all_files
        .keys()
        .filter(|p| p.starts_with(&prefix) && p.ends_with("SKILL.md"))
        .cloned()
        .collect()
}

fn parse_component_file(
    all_files: &BTreeMap<String, Vec<u8>>,
    plugin_root: &str,
    full_path_or_prefix: &str,
    kind: &str,
) -> Result<Option<NewPluginComponent>> {
    if full_path_or_prefix.ends_with('/') {
        return Ok(None);
    }
    let bytes = match all_files.get(full_path_or_prefix) {
        Some(b) => b,
        None => return Ok(None),
    };
    let text = String::from_utf8_lossy(bytes).to_string();
    let parsed = parse_markdown_frontmatter(&text)?;

    let name = parsed
        .metadata
        .as_ref()
        .and_then(|m| m.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let p = std::path::Path::new(full_path_or_prefix);
            if p.file_name().and_then(|s| s.to_str()) == Some("SKILL.md") {
                return p
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
            }
            p.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

    let description = parsed
        .metadata
        .as_ref()
        .and_then(|m| m.get("description"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let plugin_relative_path = if plugin_root.is_empty() {
        full_path_or_prefix.to_string()
    } else {
        full_path_or_prefix
            .trim_start_matches(&normalize_dir_prefix(plugin_root))
            .trim_start_matches('/')
            .to_string()
    };

    Ok(Some(NewPluginComponent {
        kind: kind.to_string(),
        path: plugin_relative_path,
        name,
        description,
        markdown_content: parsed.body,
        metadata: parsed.metadata,
    }))
}
