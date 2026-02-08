use super::domain::SkillSyncOutcome;
use crate::ports::Storage;
use anyhow::Result;
use common::domain::{archive, markdown, skill};
use common::entities::{prelude::*, skill_registry, skill_versions, skills};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

fn parse_csv_or_single(raw: &str) -> Vec<String> {
    let parts = raw
        .split(',')
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if parts.is_empty() && !raw.trim().is_empty() {
        vec![raw.trim().to_string()]
    } else {
        parts
    }
}

fn ensure_array_string(value: &Value) -> Option<Vec<String>> {
    if let Some(array) = value.as_array() {
        let values = array
            .iter()
            .filter_map(|item| item.as_str())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        return if values.is_empty() {
            None
        } else {
            Some(values)
        };
    }

    value.as_str().and_then(|raw| {
        let parsed = parse_csv_or_single(raw);
        if parsed.is_empty() {
            None
        } else {
            Some(parsed)
        }
    })
}

fn normalize_skill_metadata(frontmatter: &skill::SkillFrontmatter) -> Option<Value> {
    let mut map = frontmatter
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.as_object().cloned())
        .unwrap_or_default();

    if let Some(license) = frontmatter.license.as_ref() {
        map.insert("license".to_string(), Value::String(license.clone()));
    }

    if let Some(compatibility) = frontmatter.compatibility.as_ref() {
        let values = parse_csv_or_single(compatibility)
            .into_iter()
            .map(Value::String)
            .collect::<Vec<_>>();
        if !values.is_empty() {
            map.insert("compatibility".to_string(), Value::Array(values));
        }
    } else if let Some(existing) = map.get("compatibility").and_then(ensure_array_string) {
        map.insert(
            "compatibility".to_string(),
            Value::Array(existing.into_iter().map(Value::String).collect()),
        );
    }

    if let Some(allowed_tools) = frontmatter.allowed_tools.as_ref() {
        let values = parse_csv_or_single(allowed_tools)
            .into_iter()
            .map(Value::String)
            .collect::<Vec<_>>();
        if !values.is_empty() {
            map.insert("allowed-tools".to_string(), Value::Array(values));
        }
    } else {
        let existing_allowed = map
            .get("allowed-tools")
            .or_else(|| map.get("allowed_tools"))
            .and_then(ensure_array_string);
        if let Some(existing) = existing_allowed {
            map.insert(
                "allowed-tools".to_string(),
                Value::Array(existing.into_iter().map(Value::String).collect()),
            );
        }
    }
    map.remove("allowed_tools");

    if !map.contains_key("homepage") {
        if let Some(url) = map.get("url").and_then(|value| value.as_str()) {
            if !url.trim().is_empty() {
                map.insert(
                    "homepage".to_string(),
                    Value::String(url.trim().to_string()),
                );
            }
        }
    }

    if !map.contains_key("documentation_url") {
        if let Some(docs) = map.get("docs").and_then(|value| value.as_str()) {
            if !docs.trim().is_empty() {
                map.insert(
                    "documentation_url".to_string(),
                    Value::String(docs.trim().to_string()),
                );
            }
        }
    }

    if map.is_empty() {
        None
    } else {
        Some(Value::Object(map))
    }
}

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
    tracing::debug!(repo_name = %repo.name, "sync_standalone_skills called");
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
        tracing::info!(
            "Skill '{}': existing_version={:?}, file_hash_match={:?}, unchanged={}",
            frontmatter.name,
            existing_version.as_ref().map(|v| &v.file_hash),
            file_hash_match,
            unchanged
        );

        if unchanged {
            tracing::info!("Skipping upload for {} (unchanged)", frontmatter.name);
            continue;
        }
        let new_zip_buffer =
            tokio::task::spawn_blocking(move || archive::package_zip(&prefixed_skill_files))
                .await
                .map_err(|e| anyhow::anyhow!("Zip packaging task failed: {}", e))??;
        let s3_key = format!("skills/{}/{}.zip", frontmatter.name, version_str);
        tracing::info!(
            "Uploading skill {} ({} bytes) to s3_key={}",
            frontmatter.name,
            new_zip_buffer.len(),
            s3_key
        );
        let oss_url = s3.upload(&s3_key, new_zip_buffer).await?;
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
                normalize_skill_metadata(&frontmatter),
            )
            .await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::MockStorage;
    use common::domain::skill::SkillFrontmatter;
    use common::entities::{skill_registry, skills};
    use migration::MigratorTrait;
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, EntityTrait, PaginatorTrait,
        QueryFilter, Set,
    };
    use std::collections::{BTreeMap, HashSet};

    async fn setup_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        migration::Migrator::up(&db, None).await.unwrap();
        db
    }

    async fn insert_registry(db: &DatabaseConnection, name: &str) -> skill_registry::Model {
        skill_registry::ActiveModel {
            platform: Set(skill_registry::Platform::Github),
            owner: Set("acme".to_string()),
            name: Set(name.to_string()),
            url: Set(format!("https://github.com/acme/{}", name)),
            status: Set("active".to_string()),
            stars: Set(1),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap()
    }

    fn file_map(entries: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
        entries
            .iter()
            .map(|(path, content)| ((*path).to_string(), content.as_bytes().to_vec()))
            .collect()
    }

    fn skill_md(name: &str, version: Option<&str>) -> String {
        let mut out = format!(
            "---\nname: {}\ndescription: {} description\nlicense: MIT\ncompatibility: claude, codex\nallowed-tools: bash, rg\n",
            name, name
        );
        if let Some(v) = version {
            out.push_str(&format!("metadata:\n  version: {}\n  docs: https://docs.example/{name}\n  url: https://example.com/{name}\n", v));
        }
        out.push_str("---\n# Body\n");
        out
    }

    #[test]
    fn parse_csv_and_array_helpers_cover_edge_cases() {
        assert_eq!(parse_csv_or_single("a,b,c"), vec!["a", "b", "c"]);
        assert_eq!(parse_csv_or_single("   single   "), vec!["single"]);
        assert_eq!(parse_csv_or_single(" , , "), vec![", ,"]);

        let array_value = serde_json::json!(["bash", "rg", 3]);
        assert_eq!(
            ensure_array_string(&array_value),
            Some(vec!["bash".to_string(), "rg".to_string()])
        );
        assert_eq!(ensure_array_string(&serde_json::json!([])), None);
        assert_eq!(
            ensure_array_string(&serde_json::json!("bash, rg")),
            Some(vec!["bash".to_string(), "rg".to_string()])
        );
        assert_eq!(ensure_array_string(&serde_json::json!(null)), None);
    }

    #[test]
    fn normalize_skill_metadata_merges_known_fields() {
        let frontmatter = SkillFrontmatter {
            name: "sample-skill".to_string(),
            description: "desc".to_string(),
            license: Some("MIT".to_string()),
            compatibility: None,
            allowed_tools: None,
            metadata: Some(serde_json::json!({
                "compatibility": "claude, codex",
                "allowed_tools": ["bash", "rg"],
                "docs": "https://docs.example/skill",
                "url": "https://example.com/skill"
            })),
        };

        let normalized = normalize_skill_metadata(&frontmatter).unwrap();
        assert_eq!(normalized["license"], "MIT");
        assert_eq!(
            normalized["compatibility"],
            serde_json::json!(["claude", "codex"])
        );
        assert_eq!(
            normalized["allowed-tools"],
            serde_json::json!(["bash", "rg"])
        );
        assert_eq!(
            normalized["documentation_url"],
            serde_json::json!("https://docs.example/skill")
        );
        assert_eq!(
            normalized["homepage"],
            serde_json::json!("https://example.com/skill")
        );
        assert!(normalized.get("allowed_tools").is_none());
    }

    #[tokio::test]
    async fn sync_standalone_skills_happy_path_is_idempotent() {
        let db = setup_db().await;
        let repo = insert_registry(&db, "standalone-repo").await;

        let mut storage = MockStorage::new();
        storage.expect_upload().times(1).returning(|key, body| {
            assert!(key.starts_with("skills/demo-skill/1.0.0.zip"));
            assert!(!body.is_empty());
            Ok(format!("https://oss.local/{key}"))
        });

        let files = file_map(&[
            ("demo/SKILL.md", &skill_md("demo-skill", Some("1.0.0"))),
            ("demo/scripts/run.sh", "echo ok"),
        ]);

        let first = sync_standalone_skills(&db, &storage, &repo, &files, &HashSet::new(), true)
            .await
            .unwrap();
        assert!(first.changed);
        assert!(first.found_any);

        let second = sync_standalone_skills(&db, &storage, &repo, &files, &HashSet::new(), true)
            .await
            .unwrap();
        assert!(!second.changed);
        assert!(second.found_any);

        let skill = Skills::find()
            .filter(skills::Column::SkillRegistryId.eq(repo.id))
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(skill.name, "demo-skill");
        assert_eq!(skill.latest_version.as_deref(), Some("1.0.0"));

        let version = SkillVersions::find()
            .filter(common::entities::skill_versions::Column::SkillId.eq(skill.id))
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(version.version, "1.0.0");
        let metadata = version.metadata.unwrap();
        assert_eq!(metadata["license"], "MIT");
        assert_eq!(metadata["allowed-tools"], serde_json::json!(["bash", "rg"]));
    }

    #[tokio::test]
    async fn sync_standalone_skills_returns_not_found_when_required() {
        let db = setup_db().await;
        let repo = insert_registry(&db, "invalid-repo").await;

        let files = file_map(&[("bad/SKILL.md", "name: invalid")]);
        let outcome = sync_standalone_skills(
            &db,
            &MockStorage::new(),
            &repo,
            &files,
            &HashSet::new(),
            true,
        )
        .await
        .unwrap();

        assert!(!outcome.changed);
        assert!(!outcome.found_any);
        assert_eq!(Skills::find().count(&db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn sync_standalone_skills_deactivates_missing_skills() {
        let db = setup_db().await;
        let repo = insert_registry(&db, "deactivate-repo").await;

        let mut storage = MockStorage::new();
        storage
            .expect_upload()
            .times(2)
            .returning(|key, _| Ok(format!("https://oss.local/{key}")));

        let initial_files = file_map(&[
            ("alpha/SKILL.md", &skill_md("alpha-skill", Some("1.0.0"))),
            ("beta/SKILL.md", &skill_md("beta-skill", Some("1.0.0"))),
        ]);

        let first =
            sync_standalone_skills(&db, &storage, &repo, &initial_files, &HashSet::new(), true)
                .await
                .unwrap();
        assert!(first.changed);

        let second_files = file_map(&[("alpha/SKILL.md", &skill_md("alpha-skill", Some("1.0.0")))]);
        let second =
            sync_standalone_skills(&db, &storage, &repo, &second_files, &HashSet::new(), true)
                .await
                .unwrap();
        assert!(!second.changed);
        assert!(second.found_any);

        let beta = Skills::find()
            .filter(skills::Column::SkillRegistryId.eq(repo.id))
            .filter(skills::Column::Name.eq("beta-skill"))
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(beta.is_active, 0);
    }

    #[tokio::test]
    async fn sync_standalone_skills_respects_excluded_prefixes() {
        let db = setup_db().await;
        let repo = insert_registry(&db, "exclude-repo").await;

        let mut storage = MockStorage::new();
        storage
            .expect_upload()
            .times(1)
            .returning(|key, _| Ok(format!("https://oss.local/{key}")));

        let files = file_map(&[
            ("root/SKILL.md", &skill_md("root-skill", Some("1.0.0"))),
            (
                "plugins/p1/skills/ignored/SKILL.md",
                &skill_md("ignored-skill", Some("1.0.0")),
            ),
        ]);

        let mut excludes = HashSet::new();
        excludes.insert("plugins/p1/skills".to_string());

        let outcome = sync_standalone_skills(&db, &storage, &repo, &files, &excludes, false)
            .await
            .unwrap();
        assert!(outcome.changed);
        assert!(outcome.found_any);

        let names: Vec<String> = Skills::find()
            .filter(skills::Column::SkillRegistryId.eq(repo.id))
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert_eq!(names, vec!["root-skill".to_string()]);
    }
}
