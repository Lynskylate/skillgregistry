use crate::ports::{GithubApi, Storage};
use anyhow::Result;
use common::entities::{prelude::*, *};
use md5;
use sea_orm::*;
use serde::Deserialize;
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

/// Validates a skill according to the specification.
pub fn verify_skill(_expected_name: &str, frontmatter_str: &str) -> Result<SkillFrontmatter> {
    // 1. Parse YAML and check for unknown keys
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

    // 2. Validate name
    let n = &frontmatter.name;
    if n.len() < 1 || n.len() > 64 {
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

    // According to spec, name must match the parent directory name.
    // In our registry, we expect the frontmatter name to be at least consistent with what we discovered
    // or we might allow it if it's a valid name, but let's be strict if requested.
    // However, repo.name might be different from skill name if multiple skills in one repo.
    // The spec says "Must match the parent directory name".
    // For now, let's just ensure it's a valid name.

    // 3. Validate description
    if frontmatter.description.is_empty() || frontmatter.description.len() > 1024 {
        return Err(anyhow::anyhow!("Description must be 1-1024 characters"));
    }

    Ok(frontmatter)
}

/// Packages skill files into a ZIP buffer.
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

pub async fn run(
    db: &DatabaseConnection,
    s3: &impl Storage,
    github: &impl GithubApi,
) -> Result<()> {
    tracing::info!("Starting sync task...");

    // Clean old blacklist entries (e.g. older than 30 days)
    // This is a simple implementation, in prod could be a separate task
    let expiry_date = chrono::Utc::now().naive_utc() - chrono::Duration::days(30);
    let delete_res = Blacklist::delete_many()
        .filter(blacklist::Column::CreatedAt.lt(expiry_date))
        .exec(db)
        .await;
    if let Ok(res) = delete_res {
        if res.rows_affected > 0 {
            tracing::info!("Cleaned up {} expired blacklist entries", res.rows_affected);
        }
    }

    let repos = SkillRegistry::find().all(db).await?;

    for repo in repos {
        if let Err(e) = process_repo(db, s3, github, &repo).await {
            tracing::error!("Failed to process repo {}/{}: {}", repo.owner, repo.name, e);
        }
    }

    tracing::info!("Sync task completed.");
    Ok(())
}

async fn process_repo(
    db: &DatabaseConnection,
    s3: &impl Storage,
    github: &impl GithubApi,
    repo: &skill_registry::Model,
) -> Result<()> {
    tracing::info!("Processing repo: {}/{}", repo.owner, repo.name);

    // 1. Download zip
    let zip_data = match github.download_zipball(&repo.owner, &repo.name).await {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!(
                "Failed to download zip for {}/{}: {}",
                repo.owner,
                repo.name,
                e
            );
            return Err(e);
        }
    };

    // 2. Extract and find SKILL.md
    let reader = Cursor::new(zip_data.clone());
    let mut archive = match ZipArchive::new(reader) {
        Ok(a) => a,
        Err(e) => {
            return blacklist_repo(db, repo, &format!("Invalid zip archive: {}", e)).await;
        }
    };

    let mut skill_md_content = String::new();
    let mut skill_dir_prefix = String::new();
    let mut found = false;

    // Find SKILL.md and identify the skill directory
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        if name.ends_with("/SKILL.md") || name == "SKILL.md" {
            file.read_to_string(&mut skill_md_content)?;
            found = true;
            // Get directory prefix (e.g., "owner-repo-hash/skill-dir/")
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
        return blacklist_repo(db, repo, "No SKILL.md found").await;
    }

    // 3. Parse Frontmatter
    let parts: Vec<&str> = skill_md_content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return blacklist_repo(
            db,
            repo,
            "Invalid SKILL.md format (missing frontmatter separators)",
        )
        .await;
    }

    let frontmatter_str = parts[1];
    let body = parts[2];

    let frontmatter: SkillFrontmatter = match verify_skill(&repo.name, frontmatter_str) {
        Ok(f) => f,
        Err(e) => {
            return blacklist_repo(db, repo, &format!("Skill verification failed: {}", e)).await;
        }
    };

    // 4. Calculate Hash of the skill directory
    let mut context = md5::Context::new();

    let mut file_map: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut archive = ZipArchive::new(Cursor::new(zip_data.clone()))?; // Re-open

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        // Normalize name check
        let name_check = if skill_dir_prefix.is_empty() {
            name.clone()
        } else {
            name.clone()
        };

        if name_check.starts_with(&skill_dir_prefix) && !file.is_dir() {
            let relative_path = if skill_dir_prefix.is_empty() {
                name.clone()
            } else {
                name.trim_start_matches(&skill_dir_prefix).to_string()
            };

            // Remove any leading slash if present (zip spec says no leading slash but safe to check)
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
        // Check latest version hash
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
                        return Ok(());
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

    // 7. Repackage relevant files to a new zip
    let new_zip_buffer = package_skill(&file_map)?;

    // 8. Upload to S3
    let s3_key = format!("skills/{}/{}.zip", frontmatter.name, version_str);
    let oss_url = s3.upload(&s3_key, new_zip_buffer).await?;

    // 9. Update DB
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
        version: Set(version_str),
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

    Ok(())
}

async fn blacklist_repo(
    db: &DatabaseConnection,
    repo: &skill_registry::Model,
    reason: &str,
) -> Result<()> {
    tracing::warn!("Blacklisting repo {}/{}: {}", repo.owner, repo.name, reason);

    // Add to blacklist
    let blacklist_entry = blacklist::ActiveModel {
        repository_url: Set(repo.url.clone()),
        reason: Set(reason.to_string()),
        created_at: Set(chrono::Utc::now().naive_utc()),
        ..Default::default()
    };

    // Use insert on conflict do update to ensure we just update the reason/time if already blacklisted (though it shouldn't be)
    // SeaORM insert might fail if unique constraint violated, let's try strict insert and handle error or check first.
    // Given the flow, checking first is safer or just ignoring error.
    if let Err(e) = blacklist_entry.insert(db).await {
        tracing::warn!(
            "Failed to insert blacklist entry (might already exist): {}",
            e
        );
    }

    // Remove from skill_registry
    let res = skill_registry::Entity::delete_by_id(repo.id)
        .exec(db)
        .await?;
    if res.rows_affected > 0 {
        tracing::info!("Removed {} from skill registry", repo.name);
    }

    Ok(())
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

            zip.start_file("extra.txt", options).unwrap();
            zip.write_all(b"extra content").unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    #[tokio::test]
    async fn test_sync_new_skill() -> Result<()> {
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

        let db = db.append_query_results(vec![Vec::<skills::Model>::new()]);

        let db = db
            .append_exec_results(vec![
                // 3. Blacklist cleanup (delete_many)
                MockExecResult {
                    last_insert_id: 0,
                    rows_affected: 0,
                },
                // 4. Skills insert
                MockExecResult {
                    last_insert_id: 10,
                    rows_affected: 1,
                },
                // 5. SkillVersions insert
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

        run(&db, &s3, &github).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_sync_existing_skill_no_change() -> Result<()> {
        let zip_data = create_test_zip();

        // Calculate expected hash for comparison
        let mut keys = vec!["SKILL.md", "extra.txt"];
        keys.sort();

        let mut context = md5::Context::new();
        context.consume(b"SKILL.md");
        context.consume(
            b"---
name: test-skill
description: test description
metadata:
  version: 1.0.0
---
# Test Skill Body",
        );
        context.consume(b"extra.txt");
        context.consume(b"extra content");
        let hash = format!("{:x}", context.compute());

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

        let db = db.append_query_results(vec![vec![skills::Model {
            id: 10,
            name: "test-skill".to_string(),
            skill_registry_id: 1,
            latest_version: Some("1.0.0".to_string()),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        }]]);

        let db = db.append_query_results(vec![vec![skill_versions::Model {
            id: 100,
            skill_id: 10,
            version: "1.0.0".to_string(),
            description: None,
            readme_content: None,
            s3_key: None,
            oss_url: None,
            file_hash: Some(hash),
            metadata: None,
            created_at: chrono::Utc::now().naive_utc(),
        }]]);

        let db = db
            .append_exec_results(vec![
                // Blacklist cleanup
                MockExecResult {
                    last_insert_id: 0,
                    rows_affected: 0,
                },
            ])
            .into_connection();

        let mut github = MockGithubApi::new();
        let s3 = MockStorage::new();

        github
            .expect_download_zipball()
            .returning(move |_, _| Ok(zip_data.clone()));

        run(&db, &s3, &github).await?;

        Ok(())
    }

    #[test]
    fn test_verify_skill_valid() {
        let frontmatter = "
name: test-skill
description: valid description
license: MIT
allowed-tools: bash python
metadata:
  author: someone
";
        let res = verify_skill("test-skill", frontmatter);
        assert!(res.is_ok());
        let f = res.unwrap();
        assert_eq!(f.name, "test-skill");
        assert_eq!(f.allowed_tools.as_deref(), Some("bash python"));
    }

    #[test]
    fn test_verify_skill_invalid_name() {
        let frontmatter = "
name: Test-Skill
description: valid description
";
        let res = verify_skill("Test-Skill", frontmatter);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("lowercase"));
    }

    #[test]
    fn test_verify_skill_unexpected_key() {
        let frontmatter = "
name: test-skill
description: valid description
version: 1.0.0
";
        let res = verify_skill("test-skill", frontmatter);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Unexpected key"));
    }

    #[test]
    fn test_package_skill() {
        let mut file_map = BTreeMap::new();
        file_map.insert("SKILL.md".to_string(), b"content".to_vec());
        file_map.insert("scripts/run.py".to_string(), b"print(1)".to_vec());

        let zip_buf = package_skill(&file_map).unwrap();
        assert!(!zip_buf.is_empty());

        let reader = Cursor::new(zip_buf);
        let mut archive = ZipArchive::new(reader).unwrap();
        assert_eq!(archive.len(), 2);

        let mut s = String::new();
        archive
            .by_name("SKILL.md")
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        assert_eq!(s, "content");
    }
}
