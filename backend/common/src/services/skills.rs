use super::ServiceError;
use crate::repositories::registry::RegistryRepository;
use crate::repositories::skills::{ListSkillsParams, SkillRepository, SkillWithRegistry};
use crate::s3::S3Service;
use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Serialize)]
pub struct SkillDto {
    pub id: i32,
    pub name: String,
    pub owner: String,
    pub repo: String,
    pub host: String,
    pub latest_version: Option<String>,
    pub description: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub install_count: i32,
    pub stars: i32,
}

#[derive(Serialize)]
pub struct PaginatedSkillsResponse {
    pub items: Vec<SkillDto>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(Serialize)]
pub struct SkillDetail {
    pub skill: serde_json::Value,
    pub versions: Vec<serde_json::Value>,
    pub registry: serde_json::Value,
    pub install_count: i32,
    pub last_synced_at: Option<chrono::NaiveDateTime>,
    pub license: Option<String>,
    pub compatibility: Option<Vec<String>>,
    pub allowed_tools: Option<Vec<String>>,
    pub homepage: Option<String>,
    pub documentation_url: Option<String>,
}

#[derive(Serialize)]
pub struct SkillVersionDetail {
    pub skill_version: serde_json::Value,
}

#[derive(Serialize)]
pub struct DownloadSkillResult {
    pub download_url: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub md5: Option<String>,
    pub version: String,
    pub file_size: Option<i64>,
}

#[async_trait]
pub trait SkillService: Send + Sync {
    async fn list_skills(
        &self,
        params: ListSkillsParams<'_>,
    ) -> Result<PaginatedSkillsResponse, ServiceError>;

    async fn get_skill_by_host(
        &self,
        host: &str,
        org: &str,
        repo: &str,
        name: &str,
    ) -> Result<SkillDetail, ServiceError>;

    async fn get_skill_version_by_host(
        &self,
        host: &str,
        org: &str,
        repo: &str,
        name: &str,
        version: &str,
    ) -> Result<SkillVersionDetail, ServiceError>;

    async fn download_skill(
        &self,
        host: &str,
        org: &str,
        repo: &str,
        name: &str,
    ) -> Result<DownloadSkillResult, ServiceError>;
}

pub struct SkillServiceImpl {
    skill_repo: Arc<dyn SkillRepository>,
    registry_repo: Arc<dyn RegistryRepository>,
    s3_service: Arc<S3Service>,
}

impl SkillServiceImpl {
    pub fn new(
        skill_repo: Arc<dyn SkillRepository>,
        registry_repo: Arc<dyn RegistryRepository>,
        s3_service: Arc<S3Service>,
    ) -> Self {
        Self {
            skill_repo,
            registry_repo,
            s3_service,
        }
    }

    async fn latest_versions_map(
        &self,
        items: &[SkillWithRegistry],
    ) -> Result<HashMap<i32, crate::entities::skill_versions::Model>, ServiceError> {
        let skill_ids = items
            .iter()
            .filter(|item| item.skill.latest_version.is_some())
            .map(|item| item.skill.id)
            .collect::<Vec<_>>();
        if skill_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let versions = self
            .skill_repo
            .find_versions_for_skills(&skill_ids)
            .await
            .map_err(ServiceError::from)?;

        let mut by_key = HashMap::with_capacity(versions.len());
        for version in versions {
            by_key.insert((version.skill_id, version.version.clone()), version);
        }

        let mut latest_by_skill = HashMap::new();
        for item in items {
            if let Some(latest_version) = item.skill.latest_version.as_ref() {
                if let Some(model) = by_key.remove(&(item.skill.id, latest_version.clone())) {
                    latest_by_skill.insert(item.skill.id, model);
                }
            }
        }

        Ok(latest_by_skill)
    }

    fn extract_host(url: &str) -> String {
        let without_scheme = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .unwrap_or(url);
        without_scheme
            .split('/')
            .next()
            .unwrap_or_default()
            .to_string()
    }

    fn metadata_value<'a>(
        metadata: &'a serde_json::Value,
        key: &str,
        alternate: Option<&str>,
    ) -> Option<&'a serde_json::Value> {
        metadata
            .get(key)
            .or_else(|| alternate.and_then(|alt| metadata.get(alt)))
    }

    fn metadata_string(
        metadata: Option<&serde_json::Value>,
        key: &str,
        alternate: Option<&str>,
    ) -> Option<String> {
        metadata
            .and_then(|m| Self::metadata_value(m, key, alternate))
            .and_then(|v| v.as_str())
            .map(ToString::to_string)
    }

    fn metadata_string_array(
        metadata: Option<&serde_json::Value>,
        key: &str,
        alternate: Option<&str>,
    ) -> Option<Vec<String>> {
        let value = metadata.and_then(|m| Self::metadata_value(m, key, alternate))?;
        if let Some(items) = value.as_array() {
            let out = items
                .iter()
                .filter_map(|item| item.as_str())
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            return if out.is_empty() { None } else { Some(out) };
        }

        value.as_str().and_then(|single| {
            let out = single
                .split(',')
                .map(str::trim)
                .filter(|segment| !segment.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            if out.is_empty() {
                None
            } else {
                Some(out)
            }
        })
    }

    fn has_compatibility(metadata: Option<&serde_json::Value>, expected: &str) -> bool {
        let normalized = expected.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return true;
        }

        Self::metadata_string_array(metadata, "compatibility", None)
            .map(|entries| {
                entries
                    .iter()
                    .any(|entry| entry.trim().eq_ignore_ascii_case(&normalized))
            })
            .unwrap_or(false)
    }

    fn matches_compatibility(
        latest_version: Option<&crate::entities::skill_versions::Model>,
        compatibility: &str,
    ) -> bool {
        let metadata = latest_version.and_then(|version| version.metadata.as_ref());
        Self::has_compatibility(metadata, compatibility)
    }

    fn to_skill_dto(
        item: SkillWithRegistry,
        latest_version: Option<&crate::entities::skill_versions::Model>,
    ) -> SkillDto {
        let description = latest_version.and_then(|version| version.description.clone());
        SkillDto {
            id: item.skill.id,
            name: item.skill.name,
            owner: item.registry.owner.clone(),
            repo: item.registry.name.clone(),
            host: item
                .registry
                .host
                .clone()
                .unwrap_or_else(|| Self::extract_host(&item.registry.url)),
            latest_version: item.skill.latest_version,
            description,
            created_at: item.skill.created_at,
            install_count: item.skill.install_count,
            stars: item.registry.stars,
        }
    }
}

#[async_trait::async_trait]
impl SkillService for SkillServiceImpl {
    async fn list_skills(
        &self,
        params: ListSkillsParams<'_>,
    ) -> Result<PaginatedSkillsResponse, ServiceError> {
        let compatibility_filter = params
            .compatibility
            .map(str::trim)
            .filter(|v| !v.is_empty());

        if let Some(compatibility) = compatibility_filter {
            let requested_page = params.page.max(1);
            let requested_per_page = params.per_page.max(1);
            let start = requested_page
                .saturating_sub(1)
                .saturating_mul(requested_per_page);
            let end = start.saturating_add(requested_per_page);
            let scan_per_page = requested_per_page.max(100);

            let mut matched_total = 0_u64;
            let mut scan_page = 1_u64;
            let mut selected_items = Vec::new();

            loop {
                let batch = self
                    .skill_repo
                    .list_skills(ListSkillsParams {
                        host: params.host,
                        org: params.org,
                        owner: params.owner,
                        repo: params.repo,
                        query: params.query,
                        sort_by: params.sort_by,
                        order: params.order,
                        compatibility: None,
                        has_version: params.has_version,
                        page: scan_page,
                        per_page: scan_per_page,
                    })
                    .await?;

                if batch.items.is_empty() {
                    break;
                }

                let latest_versions = self.latest_versions_map(&batch.items).await?;
                for item in batch.items {
                    let latest = latest_versions.get(&item.skill.id);
                    if Self::matches_compatibility(latest, compatibility) {
                        if matched_total >= start && matched_total < end {
                            selected_items.push((item, latest.cloned()));
                        }
                        matched_total = matched_total.saturating_add(1);
                    }
                }

                if !batch.has_next {
                    break;
                }
                scan_page = scan_page.saturating_add(1);
            }

            let items = selected_items
                .into_iter()
                .map(|(item, latest)| Self::to_skill_dto(item, latest.as_ref()))
                .collect::<Vec<_>>();

            return Ok(PaginatedSkillsResponse {
                has_next: requested_page.saturating_mul(requested_per_page) < matched_total,
                items,
                total: matched_total,
                page: requested_page,
                per_page: requested_per_page,
            });
        }

        let paginated = self
            .skill_repo
            .list_skills(ListSkillsParams {
                host: params.host,
                org: params.org,
                owner: params.owner,
                repo: params.repo,
                query: params.query,
                sort_by: params.sort_by,
                order: params.order,
                compatibility: None,
                has_version: params.has_version,
                page: params.page,
                per_page: params.per_page,
            })
            .await?;

        let latest_versions = self.latest_versions_map(&paginated.items).await?;
        let items = paginated
            .items
            .into_iter()
            .map(|item| {
                let latest = latest_versions.get(&item.skill.id);
                Self::to_skill_dto(item, latest)
            })
            .collect::<Vec<_>>();

        Ok(PaginatedSkillsResponse {
            items,
            total: paginated.total,
            page: paginated.page,
            per_page: paginated.per_page,
            has_next: paginated.has_next,
        })
    }

    async fn get_skill_by_host(
        &self,
        host: &str,
        org: &str,
        repo: &str,
        name: &str,
    ) -> Result<SkillDetail, ServiceError> {
        let registry = self
            .registry_repo
            .find_by_host(host, org, repo)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Repository not found"))?;

        let skill = self
            .skill_repo
            .find_by_registry_name(registry.id, name)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Skill not found"))?;

        let versions = self.skill_repo.find_versions(skill.id).await?;
        let latest_version = versions
            .iter()
            .find(|v| Some(&v.version) == skill.latest_version.as_ref())
            .or_else(|| versions.first());
        let metadata = latest_version.and_then(|v| v.metadata.clone());
        let metadata_ref = metadata.as_ref();

        Ok(SkillDetail {
            skill: serde_json::to_value(&skill)
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
            versions: versions
                .into_iter()
                .map(|v| serde_json::to_value(&v))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
            registry: serde_json::to_value(&registry)
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
            install_count: skill.install_count,
            last_synced_at: Some(skill.updated_at),
            license: Self::metadata_string(metadata_ref, "license", None),
            compatibility: Self::metadata_string_array(metadata_ref, "compatibility", None),
            allowed_tools: Self::metadata_string_array(
                metadata_ref,
                "allowed-tools",
                Some("allowed_tools"),
            ),
            homepage: Self::metadata_string(metadata_ref, "homepage", Some("url")),
            documentation_url: Self::metadata_string(
                metadata_ref,
                "documentation_url",
                Some("docs"),
            ),
        })
    }

    async fn get_skill_version_by_host(
        &self,
        host: &str,
        org: &str,
        repo: &str,
        name: &str,
        version: &str,
    ) -> Result<SkillVersionDetail, ServiceError> {
        let registry = self
            .registry_repo
            .find_by_host(host, org, repo)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Repository not found"))?;

        let skill = self
            .skill_repo
            .find_by_registry_name(registry.id, name)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Skill not found"))?;

        let skill_version = self
            .skill_repo
            .find_version_by_name(skill.id, version)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Version not found"))?;

        Ok(SkillVersionDetail {
            skill_version: serde_json::to_value(&skill_version)
                .map_err(|e| ServiceError::new(500, format!("Serialization error: {}", e)))?,
        })
    }

    async fn download_skill(
        &self,
        host: &str,
        org: &str,
        repo: &str,
        name: &str,
    ) -> Result<DownloadSkillResult, ServiceError> {
        let registry = self
            .registry_repo
            .find_by_host(host, org, repo)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Repository not found"))?;

        let skill = self
            .skill_repo
            .find_by_registry_name(registry.id, name)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Skill not found"))?;

        let version = skill
            .latest_version
            .clone()
            .ok_or_else(|| ServiceError::new(404, "No version available"))?;

        let skill_version = self
            .skill_repo
            .find_version_by_name(skill.id, &version)
            .await?
            .ok_or_else(|| ServiceError::new(404, "Version not found"))?;

        let s3_key = skill_version
            .s3_key
            .as_deref()
            .ok_or_else(|| ServiceError::new(404, "No download artifact available"))?;

        let expires_in = std::time::Duration::from_secs(15 * 60);
        let download_url = self
            .s3_service
            .get_presigned_url(s3_key, expires_in)
            .await
            .map_err(|e| {
                ServiceError::new(500, format!("Failed to generate download URL: {}", e))
            })?;

        self.skill_repo
            .increment_install_count(skill.id)
            .await
            .map_err(|e| {
                ServiceError::new(500, format!("Failed to increment install count: {}", e))
            })?;

        Ok(DownloadSkillResult {
            download_url,
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(15),
            md5: skill_version.file_hash,
            version: skill_version.version,
            file_size: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{skill_registry, skill_versions, skills};
    use crate::repositories::skills::SkillWithRegistry;

    fn sample_skill_with_registry(
        host: Option<&str>,
        latest_version: Option<&str>,
    ) -> SkillWithRegistry {
        let now = chrono::Utc::now().naive_utc();
        SkillWithRegistry {
            skill: skills::Model {
                id: 1,
                name: "assistant-skill".to_string(),
                skill_registry_id: 7,
                latest_version: latest_version.map(ToString::to_string),
                install_count: 3,
                is_active: 1,
                created_at: now,
                updated_at: now,
            },
            registry: skill_registry::Model {
                id: 7,
                discovery_registry_id: None,
                platform: skill_registry::Platform::Github,
                owner: "acme".to_string(),
                name: "skills".to_string(),
                url: "https://github.example.com/acme/skills".to_string(),
                host: host.map(ToString::to_string),
                description: None,
                repo_type: Some("skill".to_string()),
                status: "active".to_string(),
                blacklist_reason: None,
                blacklisted_at: None,
                stars: 42,
                last_scanned_at: None,
                created_at: now,
                updated_at: now,
            },
        }
    }

    fn sample_version(metadata: Option<serde_json::Value>) -> skill_versions::Model {
        skill_versions::Model {
            id: 2,
            skill_id: 1,
            version: "1.2.3".to_string(),
            description: Some("sample description".to_string()),
            readme_content: Some("# Readme".to_string()),
            s3_key: Some("skills/assistant-skill.zip".to_string()),
            oss_url: None,
            file_hash: Some("abc123".to_string()),
            metadata,
            created_at: chrono::Utc::now().naive_utc(),
        }
    }

    #[test]
    fn extract_host_handles_various_urls() {
        assert_eq!(
            SkillServiceImpl::extract_host("https://github.com/org/repo"),
            "github.com"
        );
        assert_eq!(
            SkillServiceImpl::extract_host("http://ghe.example.com/org/repo"),
            "ghe.example.com"
        );
        assert_eq!(
            SkillServiceImpl::extract_host("gitea.example.com/org/repo"),
            "gitea.example.com"
        );
    }

    #[test]
    fn metadata_helpers_support_arrays_strings_and_aliases() {
        let metadata = serde_json::json!({
            "license": "MIT",
            "compatibility": ["claude", "cursor"],
            "allowed-tools": ["bash", "git"],
            "url": "https://example.com",
            "docs": "https://example.com/docs"
        });

        assert_eq!(
            SkillServiceImpl::metadata_string(Some(&metadata), "license", None),
            Some("MIT".to_string())
        );
        assert_eq!(
            SkillServiceImpl::metadata_string_array(Some(&metadata), "compatibility", None),
            Some(vec!["claude".to_string(), "cursor".to_string()])
        );
        assert_eq!(
            SkillServiceImpl::metadata_string_array(
                Some(&metadata),
                "allowed-tools",
                Some("allowed_tools")
            ),
            Some(vec!["bash".to_string(), "git".to_string()])
        );
        assert_eq!(
            SkillServiceImpl::metadata_string(Some(&metadata), "homepage", Some("url")),
            Some("https://example.com".to_string())
        );

        let csv_meta = serde_json::json!({"compatibility": "claude, cursor, "});
        assert_eq!(
            SkillServiceImpl::metadata_string_array(Some(&csv_meta), "compatibility", None),
            Some(vec!["claude".to_string(), "cursor".to_string()])
        );

        let empty_meta = serde_json::json!({"compatibility": []});
        assert_eq!(
            SkillServiceImpl::metadata_string_array(Some(&empty_meta), "compatibility", None),
            None
        );
    }

    #[test]
    fn compatibility_matching_is_case_insensitive() {
        let version = sample_version(Some(serde_json::json!({
            "compatibility": ["Claude", "Cursor"]
        })));

        assert!(SkillServiceImpl::matches_compatibility(
            Some(&version),
            "claude"
        ));
        assert!(SkillServiceImpl::matches_compatibility(
            Some(&version),
            "CURSOR"
        ));
        assert!(!SkillServiceImpl::matches_compatibility(
            Some(&version),
            "copilot"
        ));
        assert!(SkillServiceImpl::matches_compatibility(
            Some(&version),
            "   "
        ));
        assert!(!SkillServiceImpl::matches_compatibility(None, "claude"));
    }

    #[test]
    fn to_skill_dto_prefers_registry_host_and_description() {
        let item = sample_skill_with_registry(Some("git.example.com"), Some("1.2.3"));
        let latest = sample_version(Some(serde_json::json!({"license": "MIT"})));

        let dto = SkillServiceImpl::to_skill_dto(item, Some(&latest));
        assert_eq!(dto.host, "git.example.com");
        assert_eq!(dto.latest_version.as_deref(), Some("1.2.3"));
        assert_eq!(dto.description.as_deref(), Some("sample description"));
        assert_eq!(dto.owner, "acme");
        assert_eq!(dto.repo, "skills");
    }

    #[test]
    fn to_skill_dto_falls_back_to_url_host() {
        let item = sample_skill_with_registry(None, None);
        let dto = SkillServiceImpl::to_skill_dto(item, None);

        assert_eq!(dto.host, "github.example.com");
        assert_eq!(dto.description, None);
        assert_eq!(dto.latest_version, None);
    }
}
