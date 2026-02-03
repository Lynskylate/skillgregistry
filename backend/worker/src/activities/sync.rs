use crate::ports::{GithubApi, Storage};
use anyhow::Result;
use common::entities::{prelude::*, *};
use md5;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use zip::ZipArchive;

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
    pub status: String, // "Updated", "Unchanged", "Error"
    pub version: Option<String>,
}

pub fn verify_skill(_expected_name: &str, frontmatter_str: &str) -> Result<SkillFrontmatter> {
    let yaml_value: serde_yaml::Value = serde_yaml::from_str(frontmatter_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse YAML: {}", e))?;

    let obj = yaml_value
        .as_mapping()
        .ok_or_else(|| anyhow::anyhow!("Frontmatter is not a YAML mapping"))?;

    let allowed_keys = [
        "name",
        "description",
        "license",
        "compatibility",
        "allowed-tools",
        "metadata",
    ];
    for key in obj.keys() {
        if let Some(k) = key.as_str() {
            if !allowed_keys.contains(&k) {
                return Err(anyhow::anyhow!("Unexpected key in frontmatter: {}", k));
            }
        }
    }

    let frontmatter: SkillFrontmatter = serde_yaml::from_value(yaml_value)?;

    let n = &frontmatter.name;
    if n.is_empty() || n.len() > 64 {
        return Err(anyhow::anyhow!("Skill name must be 1-64 characters"));
    }
    if !n
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow::anyhow!(
            "Skill name must only contain lowercase letters, numbers, and hyphens"
        ));
    }
    if n.starts_with('-') || n.ends_with('-') {
        return Err(anyhow::anyhow!(
            "Skill name must not start or end with a hyphen"
        ));
    }
    if n.contains("--") {
        return Err(anyhow::anyhow!(
            "Skill name must not contain consecutive hyphens"
        ));
    }

    if frontmatter.description.is_empty() || frontmatter.description.len() > 1024 {
        return Err(anyhow::anyhow!("Description must be 1-1024 characters"));
    }

    Ok(frontmatter)
}

pub fn package_skill(file_map: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    let mut new_zip_buffer = Vec::new();
    {
        let mut zip_writer = zip::ZipWriter::new(std::io::Cursor::new(&mut new_zip_buffer));
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        for (path, content) in file_map {
            zip_writer.start_file(path, options)?;
            zip_writer.write_all(content)?;
        }
        zip_writer.finish()?;
    }
    Ok(new_zip_buffer)
}

pub struct SyncService;

impl SyncService {
    pub async fn fetch_pending(db: &DatabaseConnection) -> Result<Vec<i32>> {
        // Clean old blacklist entries
        let expiry_date = chrono::Utc::now().naive_utc() - chrono::Duration::days(30);
        let _ = Blacklist::delete_many()
            .filter(blacklist::Column::CreatedAt.lt(expiry_date))
            .exec(db)
            .await;

        let repos = SkillRegistry::find().all(db).await?;
        Ok(repos.into_iter().map(|r| r.id).collect())
    }

    pub async fn process_one(
        db: &DatabaseConnection,
        s3: &impl Storage,
        github: &impl GithubApi,
        registry_id: i32,
    ) -> Result<SyncResult> {
        let repo = SkillRegistry::find_by_id(registry_id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Registry entry not found"))?;

        tracing::info!("Processing repo: {}/{}", repo.owner, repo.name);

        // 1. Download zip
        let zip_data = match github.download_zipball(&repo.owner, &repo.name).await {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!("Failed to download zip: {}", e);
                return Err(e);
            }
        };

        // 2. Extract and find SKILL.md
        let reader = Cursor::new(zip_data.clone());
        let mut archive = match ZipArchive::new(reader) {
            Ok(a) => a,
            Err(e) => {
                Self::blacklist_repo(db, &repo, &format!("Invalid zip archive: {}", e)).await?;
                return Err(anyhow::anyhow!("Invalid zip archive"));
            }
        };

        let mut skill_md_content = String::new();
        let mut skill_dir_prefix = String::new();
        let mut found = false;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();

            if name.ends_with("/SKILL.md") || name == "SKILL.md" {
                file.read_to_string(&mut skill_md_content)?;
                found = true;
                if let Some(parent) = std::path::Path::new(&name).parent() {
                    skill_dir_prefix = parent.to_string_lossy().to_string();
                    if !skill_dir_prefix.ends_with('/') && !skill_dir_prefix.is_empty() {
                        skill_dir_prefix.push('/');
                    }
                }
                break;
            }
        }

        if !found {
            Self::blacklist_repo(db, &repo, "No SKILL.md found").await?;
            return Err(anyhow::anyhow!("No SKILL.md found"));
        }

        // 3. Parse Frontmatter
        let parts: Vec<&str> = skill_md_content.splitn(3, "---").collect();
        if parts.len() < 3 {
            Self::blacklist_repo(
                db,
                &repo,
                "Invalid SKILL.md format (missing frontmatter separators)",
            )
            .await?;
            return Err(anyhow::anyhow!("Invalid SKILL.md format"));
        }

        let frontmatter_str = parts[1];
        let body = parts[2];

        let frontmatter: SkillFrontmatter = match verify_skill(&repo.name, frontmatter_str) {
            Ok(f) => f,
            Err(e) => {
                Self::blacklist_repo(db, &repo, &format!("Skill verification failed: {}", e))
                    .await?;
                return Err(e);
            }
        };

        // 4. Calculate Hash
        let mut context = md5::Context::new();
        let mut file_map: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut archive = ZipArchive::new(Cursor::new(zip_data.clone()))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();

            if name.starts_with(&skill_dir_prefix) && !file.is_dir() {
                let relative_path = if skill_dir_prefix.is_empty() {
                    name.clone()
                } else {
                    name.trim_start_matches(&skill_dir_prefix).to_string()
                };
                let relative_path = relative_path.trim_start_matches('/').to_string();

                if relative_path.is_empty() {
                    continue;
                }

                let mut content = Vec::new();
                file.read_to_end(&mut content)?;
                file_map.insert(relative_path, content);
            }
        }

        for (path, content) in &file_map {
            context.consume(path.as_bytes());
            context.consume(content);
        }
        let hash_result = context.compute();
        let hash_string = format!("{:x}", hash_result);

        // 5. Check if changed
        let existing_skill = Skills::find()
            .filter(skills::Column::Name.eq(&frontmatter.name))
            .one(db)
            .await?;

        if let Some(skill) = &existing_skill {
            if let Some(latest_v) = &skill.latest_version {
                let version = SkillVersions::find()
                    .filter(skill_versions::Column::SkillId.eq(skill.id))
                    .filter(skill_versions::Column::Version.eq(latest_v))
                    .one(db)
                    .await?;

                if let Some(v) = version {
                    if let Some(h) = v.file_hash {
                        if h == hash_string {
                            tracing::info!("Skill {} unchanged (hash match)", frontmatter.name);
                            return Ok(SyncResult {
                                status: "Unchanged".to_string(),
                                version: Some(latest_v.clone()),
                            });
                        }
                    }
                }
            }
        }

        // 6. Create New Version
        let version_str = frontmatter
            .metadata
            .as_ref()
            .and_then(|m| m.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("0.0.{}", chrono::Utc::now().timestamp()));

        tracing::info!(
            "Updating skill {} to version {}",
            frontmatter.name,
            version_str
        );

        let new_zip_buffer = package_skill(&file_map)?;

        let s3_key = format!("skills/{}/{}.zip", frontmatter.name, version_str);
        let oss_url = s3.upload(&s3_key, new_zip_buffer).await?;

        // 7. Update DB
        let skill_id = if let Some(s) = existing_skill {
            let mut active: skills::ActiveModel = s.into();
            active.updated_at = Set(chrono::Utc::now().naive_utc());
            active.latest_version = Set(Some(version_str.clone()));
            active.update(db).await?.id
        } else {
            let new_skill = skills::ActiveModel {
                name: Set(frontmatter.name.clone()),
                skill_registry_id: Set(repo.id),
                latest_version: Set(Some(version_str.clone())),
                created_at: Set(chrono::Utc::now().naive_utc()),
                updated_at: Set(chrono::Utc::now().naive_utc()),
                ..Default::default()
            };
            new_skill.insert(db).await?.id
        };

        let new_version = skill_versions::ActiveModel {
            skill_id: Set(skill_id),
            version: Set(version_str.clone()),
            description: Set(Some(frontmatter.description)),
            readme_content: Set(Some(body.to_string())),
            s3_key: Set(Some(s3_key)),
            oss_url: Set(Some(oss_url)),
            file_hash: Set(Some(hash_string)),
            metadata: Set(frontmatter.metadata),
            created_at: Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        };

        new_version.insert(db).await?;

        Ok(SyncResult {
            status: "Updated".to_string(),
            version: Some(version_str),
        })
    }

    async fn blacklist_repo(
        db: &DatabaseConnection,
        repo: &skill_registry::Model,
        reason: &str,
    ) -> Result<()> {
        tracing::warn!("Blacklisting repo {}/{}: {}", repo.owner, repo.name, reason);

        let blacklist_entry = blacklist::ActiveModel {
            repository_url: Set(repo.url.clone()),
            reason: Set(reason.to_string()),
            created_at: Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        };

        if let Err(e) = blacklist_entry.insert(db).await {
            tracing::warn!("Failed to insert blacklist entry: {}", e);
        }

        let _ = skill_registry::Entity::delete_by_id(repo.id)
            .exec(db)
            .await?;

        Ok(())
    }
}

use temporalio_sdk::{ActContext, ActivityError};

pub async fn fetch_pending_skills_activity(
    _ctx: ActContext,
    _input: (),
) -> Result<Vec<i32>, ActivityError> {
    let state = crate::get_app_state().await;
    SyncService::fetch_pending(&state.db)
        .await
        .map_err(ActivityError::from)
}

pub async fn sync_single_skill_activity(
    _ctx: ActContext,
    registry_id: i32,
) -> Result<SyncResult, ActivityError> {
    let state = crate::get_app_state().await;
    SyncService::process_one(&state.db, &state.s3, &state.github, registry_id)
        .await
        .map_err(ActivityError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{MockGithubApi, MockStorage};
    use common::entities::{skill_registry, skill_versions, skills};
    use sea_orm::DatabaseBackend;
    use std::io::Write;
    use zip::write::FileOptions;

    fn create_test_zip() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);

            zip.start_file("SKILL.md", options).unwrap();
            zip.write_all(
                b"---
name: test-skill
description: test description
metadata:
  version: 1.0.0
---
# Test Skill Body",
            )
            .unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    #[tokio::test]
    async fn test_sync_service_new_skill() -> Result<()> {
        let db = MockDatabase::new(DatabaseBackend::Sqlite).append_query_results(vec![vec![
            skill_registry::Model {
                id: 1,
                platform: skill_registry::Platform::Github,
                owner: "test-owner".to_string(),
                name: "test-repo".to_string(),
                url: "https://github.com/test-owner/test-repo".to_string(),
                description: None,
                stars: 0,
                created_at: chrono::Utc::now().naive_utc(),
                updated_at: chrono::Utc::now().naive_utc(),
                last_scanned_at: None,
            },
        ]]);

        let db = db.append_query_results(vec![Vec::<skills::Model>::new()]); // Existing skill check (None)
        let db = db.append_query_results(vec![Vec::<skills::Model>::new()]); // Extra buffer just in case

        let db = db
            .append_exec_results(vec![
                // Skills insert
                MockExecResult {
                    last_insert_id: 10,
                    rows_affected: 1,
                },
                // SkillVersions insert
                MockExecResult {
                    last_insert_id: 100,
                    rows_affected: 1,
                },
            ])
            .into_connection();

        let mut github = MockGithubApi::new();
        let mut s3 = MockStorage::new();

        github
            .expect_download_zipball()
            .returning(|_, _| Ok(create_test_zip()));

        s3.expect_upload()
            .returning(|_, _| Ok("https://oss.com/test.zip".to_string()));

        let res = SyncService::process_one(&db, &s3, &github, 1).await?;
        assert_eq!(res.status, "Updated");

        Ok(())
    }
}
