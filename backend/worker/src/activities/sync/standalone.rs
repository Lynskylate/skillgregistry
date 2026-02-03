use super::domain::SkillSyncOutcome;
use super::utils::{
    compute_hash, normalize_dir_prefix, package_skill, split_raw_frontmatter, subtree_file_map,
    verify_skill,
};
use crate::ports::Storage;
use anyhow::Result;
use common::entities::{prelude::*, *};
use sea_orm::*;
use std::collections::{BTreeMap, HashSet};

pub async fn sync_standalone_skills(
    db: &DatabaseConnection,
    s3: &impl Storage,
    repo: &skill_registry::Model,
    all_files: &BTreeMap<String, Vec<u8>>,
    exclude_prefixes: &HashSet<String>,
    require_any_valid: bool,
) -> Result<SkillSyncOutcome> {
    let mut candidate_paths: Vec<String> = all_files
        .keys()
        .filter(|p| p.ends_with("SKILL.md"))
        .cloned()
        .collect();
    candidate_paths.sort();

    let normalized_excludes: Vec<String> = exclude_prefixes
        .iter()
        .map(|p| normalize_dir_prefix(p))
        .filter(|p| !p.is_empty())
        .collect();

    let mut found_skill_names = HashSet::new();
    let mut changed = false;

    for path in candidate_paths {
        if normalized_excludes.iter().any(|ex| path.starts_with(ex)) {
            continue;
        }

        let content = match all_files.get(&path) {
            Some(c) => c,
            None => continue,
        };
        let md = String::from_utf8_lossy(content).to_string();

        let (frontmatter_raw, body) = match split_raw_frontmatter(&md) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let frontmatter = match verify_skill(&repo.name, &frontmatter_raw) {
            Ok(f) => f,
            Err(_) => continue,
        };

        if found_skill_names.contains(&frontmatter.name) {
            continue;
        }
        found_skill_names.insert(frontmatter.name.clone());

        let skill_dir = std::path::Path::new(&path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let skill_files = subtree_file_map(all_files, &skill_dir);
        let hash_string = compute_hash(&skill_files);

        let version_str = frontmatter
            .metadata
            .as_ref()
            .and_then(|m| m.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("0.0.{}", chrono::Utc::now().timestamp()));

        let existing_skill = Skills::find()
            .filter(skills::Column::SkillRegistryId.eq(repo.id))
            .filter(skills::Column::Name.eq(&frontmatter.name))
            .one(db)
            .await?;

        let skill_id = if let Some(s) = existing_skill {
            let mut active: skills::ActiveModel = s.into();
            active.updated_at = Set(chrono::Utc::now().naive_utc());
            active.latest_version = Set(Some(version_str.clone()));
            active.is_active = Set(1);
            active.update(db).await?.id
        } else {
            let new_skill = skills::ActiveModel {
                name: Set(frontmatter.name.clone()),
                skill_registry_id: Set(repo.id),
                latest_version: Set(Some(version_str.clone())),
                is_active: Set(1),
                created_at: Set(chrono::Utc::now().naive_utc()),
                updated_at: Set(chrono::Utc::now().naive_utc()),
                ..Default::default()
            };
            new_skill.insert(db).await?.id
        };

        let existing_version = SkillVersions::find()
            .filter(skill_versions::Column::SkillId.eq(skill_id))
            .filter(skill_versions::Column::Version.eq(&version_str))
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

        let new_zip_buffer = package_skill(&skill_files)?;
        let s3_key = format!("skills/{}/{}.zip", frontmatter.name, version_str);
        let oss_url = s3.upload(&s3_key, new_zip_buffer).await?;

        if let Some(v) = existing_version {
            let mut active: skill_versions::ActiveModel = v.into();
            active.description = Set(Some(frontmatter.description.clone()));
            active.readme_content = Set(Some(body.clone()));
            active.s3_key = Set(Some(s3_key));
            active.oss_url = Set(Some(oss_url));
            active.file_hash = Set(Some(hash_string));
            active.metadata = Set(frontmatter.metadata.clone());
            let _ = active.update(db).await?;
        } else {
            let new_version = skill_versions::ActiveModel {
                skill_id: Set(skill_id),
                version: Set(version_str.clone()),
                description: Set(Some(frontmatter.description.clone())),
                readme_content: Set(Some(body.clone())),
                s3_key: Set(Some(s3_key)),
                oss_url: Set(Some(oss_url)),
                file_hash: Set(Some(hash_string)),
                metadata: Set(frontmatter.metadata.clone()),
                created_at: Set(chrono::Utc::now().naive_utc()),
                ..Default::default()
            };
            new_version.insert(db).await?;
        }

        changed = true;
    }

    if require_any_valid && found_skill_names.is_empty() {
        return Ok(SkillSyncOutcome {
            changed,
            found_any: false,
        });
    }

    if !found_skill_names.is_empty() {
        let existing = Skills::find()
            .filter(skills::Column::SkillRegistryId.eq(repo.id))
            .all(db)
            .await?;
        for s in existing {
            let should_be_active = found_skill_names.contains(&s.name);
            let target = if should_be_active { 1 } else { 0 };
            if s.is_active != target {
                let mut active: skills::ActiveModel = s.into();
                active.is_active = Set(target);
                active.updated_at = Set(chrono::Utc::now().naive_utc());
                let _ = active.update(db).await?;
            }
        }
    }

    Ok(SkillSyncOutcome {
        changed,
        found_any: !found_skill_names.is_empty(),
    })
}
