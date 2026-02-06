use super::domain::SkillSyncOutcome;
use crate::ports::Storage;
use anyhow::Result;
use common::domain::{archive, markdown, skill};
use common::entities::{prelude::*, skill_registry, skill_versions, skills};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use std::collections::{BTreeMap, HashSet};

struct SkillRepoHelper<'a> {
    db: &'a DatabaseConnection,
}

#[allow(clippy::too_many_arguments)]
impl<'a> SkillRepoHelper<'a> {
    async fn find_skill(
        &self,
        registry_id: i32,
        name: &str,
    ) -> Result<Option<skills::Model>, sea_orm::DbErr> {
        let result: Option<skills::Model> = Skills::find()
            .filter(skills::Column::SkillRegistryId.eq(registry_id))
            .filter(skills::Column::Name.eq(name))
            .one(self.db)
            .await?;
        Ok(result)
    }

    async fn upsert_skill(
        &self,
        existing: Option<skills::Model>,
        registry_id: i32,
        name: &str,
        latest_version: Option<String>,
        is_active: i32,
    ) -> Result<i32, sea_orm::DbErr> {
        if let Some(existing) = existing {
            let id = existing.id;
            let mut updated = skills::ActiveModel::from(existing);
            updated.latest_version = Set(latest_version);
            updated.is_active = Set(is_active);
            updated.updated_at = Set(chrono::Utc::now().naive_utc());
            updated.update(self.db).await?;
            Ok(id)
        } else {
            let now = chrono::Utc::now().naive_utc();
            let new_skill = skills::ActiveModel {
                skill_registry_id: Set(registry_id),
                name: Set(name.to_string()),
                latest_version: Set(latest_version),
                is_active: Set(is_active),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            };
            let inserted = new_skill.insert(self.db).await?;
            Ok(inserted.id)
        }
    }

    async fn find_version_by_name(
        &self,
        skill_id: i32,
        version: &str,
    ) -> Result<Option<skill_versions::Model>, sea_orm::DbErr> {
        let result: Option<skill_versions::Model> = SkillVersions::find()
            .filter(skill_versions::Column::SkillId.eq(skill_id))
            .filter(skill_versions::Column::Version.eq(version))
            .one(self.db)
            .await?;
        Ok(result)
    }

    async fn upsert_skill_version(
        &self,
        existing: Option<skill_versions::Model>,
        skill_id: i32,
        version: &str,
        description: Option<String>,
        readme_content: Option<String>,
        s3_key: Option<String>,
        oss_url: Option<String>,
        file_hash: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<(), sea_orm::DbErr> {
        if let Some(existing) = existing {
            let mut updated = skill_versions::ActiveModel::from(existing);
            updated.description = Set(description);
            updated.readme_content = Set(readme_content);
            updated.s3_key = Set(s3_key);
            updated.oss_url = Set(oss_url);
            updated.file_hash = Set(file_hash);
            updated.metadata = Set(metadata);
            updated.update(self.db).await?;
        } else {
            let now = chrono::Utc::now().naive_utc();
            let new_version = skill_versions::ActiveModel {
                skill_id: Set(skill_id),
                version: Set(version.to_string()),
                description: Set(description),
                readme_content: Set(readme_content),
                s3_key: Set(s3_key),
                oss_url: Set(oss_url),
                file_hash: Set(file_hash),
                metadata: Set(metadata),
                created_at: Set(now),
                ..Default::default()
            };
            new_version.insert(self.db).await?;
        }
        Ok(())
    }

    async fn list_skills_by_registry_id(
        &self,
        registry_id: i32,
    ) -> Result<Vec<skills::Model>, sea_orm::DbErr> {
        let result: Vec<skills::Model> = Skills::find()
            .filter(skills::Column::SkillRegistryId.eq(registry_id))
            .all(self.db)
            .await?;
        Ok(result)
    }

    async fn update_skill_active(
        &self,
        skill: skills::Model,
        is_active: i32,
    ) -> Result<(), sea_orm::DbErr> {
        let mut updated = skills::ActiveModel::from(skill);
        updated.is_active = Set(is_active);
        updated.update(self.db).await?;
        Ok(())
    }
}

pub async fn sync_standalone_skills(
    db: &DatabaseConnection,
    s3: &dyn Storage,
    repo: &skill_registry::Model,
    all_files: &BTreeMap<String, Vec<u8>>,
    exclude_prefixes: &HashSet<String>,
    require_any_valid: bool,
) -> Result<SkillSyncOutcome> {
    std::fs::write(
        "/tmp/worker_debug.txt",
        format!(
            "[STANDALONE] sync_standalone_skills called with repo.name={}\n",
            repo.name
        ),
    )
    .ok();
    eprintln!(
        "[STANDALONE] sync_standalone_skills called with repo.name={}",
        repo.name
    );
    let repo_store = SkillRepoHelper { db };
    let mut candidate_paths: Vec<String> = all_files
        .keys()
        .filter(|p| p.ends_with("SKILL.md"))
        .cloned()
        .collect();
    candidate_paths.sort();

    tracing::info!(
        "sync_standalone_skills: found {} SKILL.md files in repo {}",
        candidate_paths.len(),
        repo.name
    );
    eprintln!(
        "[STANDALONE] Found {} SKILL.md files in repo {}",
        candidate_paths.len(),
        repo.name
    );

    let normalized_excludes: Vec<String> = exclude_prefixes
        .iter()
        .map(|p| archive::normalize_dir_prefix(p))
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
            None => {
                tracing::debug!(path = %path, "Skipping SKILL.md because content was missing from file map");
                continue;
            }
        };
        let md = String::from_utf8_lossy(content).to_string();

        let (frontmatter_raw, body) = match markdown::split_raw_frontmatter(&md) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(path = %path, error = %e, "Skipping SKILL.md due to frontmatter parse failure");
                continue;
            }
        };

        let frontmatter = match skill::verify_skill(&repo.name, &frontmatter_raw) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(path = %path, error = %e, "Skipping SKILL.md due to failed verification");
                continue;
            }
        };

        if found_skill_names.contains(&frontmatter.name) {
            tracing::warn!(
                path = %path,
                skill_name = %frontmatter.name,
                "Duplicate skill name found in repo; skipping"
            );
            continue;
        }
        found_skill_names.insert(frontmatter.name.clone());

        let skill_dir = std::path::Path::new(&path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let skill_files = archive::subtree_file_map(all_files, &skill_dir);
        let content_hash = archive::compute_hash(&skill_files);
        let prefix = frontmatter.name.trim_matches('/');
        let mut prefixed_skill_files = BTreeMap::new();
        for (path, bytes) in skill_files {
            if path.is_empty() {
                continue;
            }
            let prefixed_path = if prefix.is_empty() {
                path
            } else {
                format!("{}/{}", prefix, path.trim_start_matches('/'))
            };
            prefixed_skill_files.insert(prefixed_path, bytes);
        }
        let package_hash = archive::compute_hash(&prefixed_skill_files);

        let derived_patch = match u32::from_str_radix(&content_hash[..8], 16) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    hash_prefix = %(&content_hash[..8]),
                    error = %e,
                    "Failed to derive patch version from hash prefix"
                );
                0
            }
        };
        let version_str = frontmatter
            .metadata
            .as_ref()
            .and_then(|m| m.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("0.0.{}", derived_patch));

        let existing_skill = repo_store.find_skill(repo.id, &frontmatter.name).await?;
        eprintln!(
            "[STANDALONE] Processing skill '{}': existing_skill={:?}, version={}",
            frontmatter.name,
            existing_skill.is_some(),
            version_str
        );
        tracing::info!(
            "Skill '{}': existing_skill={:?}, version={}",
            frontmatter.name,
            existing_skill.is_some(),
            version_str
        );

        let skill_id = repo_store
            .upsert_skill(
                existing_skill,
                repo.id,
                &frontmatter.name,
                Some(version_str.clone()),
                1,
            )
            .await?;

        let existing_version = repo_store
            .find_version_by_name(skill_id, &version_str)
            .await?;

        let unchanged = existing_version
            .as_ref()
            .and_then(|v| v.file_hash.as_ref())
            .map(|h| h == &package_hash)
            .unwrap_or(false);

        let file_hash_match = existing_version
            .as_ref()
            .and_then(|v| v.file_hash.as_ref())
            .map(|h| h == &package_hash);

        // Force debug output to file
        let _ = std::fs::write(
            "/tmp/worker_debug_unchanged_check.txt",
            format!("Skill '{}': existing_version={:?}, file_hash_match={:?}, unchanged={}, skill_id={}, version_str={}\n",
                    frontmatter.name,
                    existing_version.as_ref().map(|v| &v.file_hash),
                    file_hash_match,
                    unchanged,
                    skill_id,
                    version_str)
        );

        eprintln!(
            "[STANDALONE] Skill '{}': existing_version={:?}, file_hash_match={:?}, unchanged={}",
            frontmatter.name,
            existing_version.as_ref().map(|v| &v.file_hash),
            file_hash_match,
            unchanged
        );
        tracing::info!(
            "Skill '{}': existing_version={:?}, file_hash_match={:?}, unchanged={}",
            frontmatter.name,
            existing_version.as_ref().map(|v| &v.file_hash),
            file_hash_match,
            unchanged
        );

        if unchanged {
            eprintln!(
                "[STANDALONE] Skipping upload for {} (unchanged)",
                frontmatter.name
            );
            tracing::info!("Skipping upload for {} (unchanged)", frontmatter.name);
            std::fs::write(
                "/tmp/worker_debug_upload_skipped.txt",
                format!("Upload skipped for {}\n", frontmatter.name),
            )
            .ok();
            continue;
        }

        std::fs::write(
            "/tmp/worker_debug_upload_attempt.txt",
            format!("About to upload {}\n", frontmatter.name),
        )
        .ok();
        let new_zip_buffer =
            tokio::task::spawn_blocking(move || archive::package_zip(&prefixed_skill_files))
                .await
                .map_err(|e| anyhow::anyhow!("Zip packaging task failed: {}", e))??;
        let s3_key = format!("skills/{}/{}.zip", frontmatter.name, version_str);
        eprintln!(
            "[STANDALONE] About to upload skill {} ({} bytes) to s3_key={}",
            frontmatter.name,
            new_zip_buffer.len(),
            s3_key
        );
        tracing::info!(
            "Uploading skill {} ({} bytes) to s3_key={}",
            frontmatter.name,
            new_zip_buffer.len(),
            s3_key
        );
        let oss_url = s3.upload(&s3_key, new_zip_buffer).await?;
        eprintln!(
            "[STANDALONE] Upload complete for {}: oss_url={}",
            frontmatter.name, oss_url
        );
        tracing::info!(
            "Upload complete for {}: oss_url={}",
            frontmatter.name,
            oss_url
        );

        repo_store
            .upsert_skill_version(
                existing_version,
                skill_id,
                &version_str,
                Some(frontmatter.description.clone()),
                Some(body.clone()),
                Some(s3_key.clone()),
                Some(oss_url.clone()),
                Some(package_hash.clone()),
                frontmatter.metadata.clone(),
            )
            .await?;

        eprintln!(
            "[STANDALONE] After upsert_skill_version - s3_key={}, oss_url={}, file_hash={}",
            s3_key, oss_url, package_hash
        );
        changed = true;
    }

    if require_any_valid && found_skill_names.is_empty() {
        return Ok(SkillSyncOutcome {
            changed,
            found_any: false,
        });
    }

    if !found_skill_names.is_empty() {
        let existing = repo_store.list_skills_by_registry_id(repo.id).await?;
        for s in existing {
            let should_be_active = found_skill_names.contains(&s.name);
            let target = if should_be_active { 1 } else { 0 };
            if s.is_active != target {
                repo_store.update_skill_active(s, target).await?;
            }
        }
    }

    eprintln!(
        "[STANDALONE] sync_standalone_skills returning: changed={}, found_any={}",
        changed,
        !found_skill_names.is_empty()
    );
    tracing::info!(
        "sync_standalone_skills returning: changed={}, found_any={}",
        changed,
        !found_skill_names.is_empty()
    );
    Ok(SkillSyncOutcome {
        changed,
        found_any: !found_skill_names.is_empty(),
    })
}
