use anyhow::Result;
use chrono::{DateTime, Utc};
use common::infra::github_http::{build_github_client, send_request_with_retry};
use reqwest::Client;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use tokio::process::Command;
use url::Url;
use walkdir::WalkDir;

#[derive(Deserialize, Debug)]
pub struct GithubSearchResponse {
    pub total_count: u32,
    pub items: Vec<GithubRepo>,
}

#[derive(Deserialize, Debug)]
pub struct GithubCodeSearchResponse {
    pub total_count: u32,
    pub items: Vec<GithubCodeItem>,
}

#[derive(Deserialize, Debug)]
pub struct GithubCodeItem {
    pub repository: GithubRepo,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GithubRepo {
    pub name: String,
    pub html_url: String,
    pub description: Option<String>,
    #[serde(default)]
    pub stargazers_count: i32,
    #[serde(default = "utc_now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "utc_now")]
    pub updated_at: DateTime<Utc>,
    pub owner: GithubOwner,
}

fn utc_now() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Deserialize, Debug, Clone)]
pub struct GithubOwner {
    pub login: String,
}

pub struct GithubClient {
    client: Client,
    api_url: String,
}

impl GithubClient {
    pub fn new(token: Option<String>, api_url: String) -> Result<Self> {
        let client = build_github_client(token.as_deref())?;
        Ok(Self { client, api_url })
    }

    pub async fn search_repositories(&self, query: &str) -> Result<Vec<GithubRepo>> {
        let mut all_repos = Vec::new();
        let mut page = 1;
        let per_page = 100;

        loop {
            let url = format!("{}/search/repositories", self.api_url);
            tracing::debug!(page, query, "Fetching GitHub repository search page");

            let per_page_str = per_page.to_string();
            let page_str = page.to_string();
            let req = self.client.get(&url).query(&[
                ("q", query),
                ("per_page", per_page_str.as_str()),
                ("page", page_str.as_str()),
            ]);
            let resp = send_request_with_retry(req, "worker search repositories").await?;
            let search_resp: GithubSearchResponse = resp.json().await?;

            if search_resp.items.is_empty() {
                break;
            }

            all_repos.extend(search_resp.items);

            if all_repos.len() >= search_resp.total_count as usize || all_repos.len() >= 1000 {
                break;
            }

            page += 1;
        }

        Ok(all_repos)
    }

    pub async fn search_code(&self, query: &str) -> Result<Vec<GithubRepo>> {
        let mut all_repos = Vec::new();
        let mut page = 1;
        let per_page = 100;

        loop {
            let url = format!("{}/search/code", self.api_url);
            tracing::debug!(page, query, "Fetching GitHub code search page");

            let per_page_str = per_page.to_string();
            let page_str = page.to_string();
            let req = self.client.get(&url).query(&[
                ("q", query),
                ("per_page", per_page_str.as_str()),
                ("page", page_str.as_str()),
            ]);
            let resp = send_request_with_retry(req, "worker search code").await?;
            let search_resp: GithubCodeSearchResponse = resp.json().await?;

            if search_resp.items.is_empty() {
                break;
            }

            for item in search_resp.items {
                all_repos.push(item.repository);
            }

            if all_repos.len() >= search_resp.total_count as usize || all_repos.len() >= 1000 {
                break;
            }

            page += 1;
        }

        Ok(all_repos)
    }

    pub async fn download_zipball(&self, owner: &str, repo: &str) -> Result<Vec<u8>> {
        let url = format!("{}/repos/{}/{}/zipball", self.api_url, owner, repo);
        let req = self.client.get(&url);
        let resp = send_request_with_retry(req, "worker download zipball").await?;
        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }

    pub async fn clone_repository_files(
        &self,
        owner: &str,
        repo: &str,
        repo_url: &str,
        token: Option<String>,
    ) -> Result<BTreeMap<String, Vec<u8>>> {
        let temp_dir = tempfile::tempdir()?;
        let checkout_dir = temp_dir.path().join("repo");
        let clone_url = Self::build_clone_url(owner, repo, repo_url);

        let output = Self::run_git_clone(&clone_url, &checkout_dir, token.as_deref()).await?;
        if output.status.success() {
            return collect_repository_files(&checkout_dir);
        }

        let primary_error = Self::sanitize_git_stderr(&output.stderr, token.as_deref());
        if token.is_some() && Self::should_retry_without_token(&primary_error) {
            tracing::warn!(
                owner,
                repo,
                "git clone with token failed, retrying without token"
            );
            if checkout_dir.exists() {
                let _ = std::fs::remove_dir_all(&checkout_dir);
            }

            let fallback_output = Self::run_git_clone(&clone_url, &checkout_dir, None).await?;
            if fallback_output.status.success() {
                return collect_repository_files(&checkout_dir);
            }

            let fallback_error = Self::sanitize_git_stderr(&fallback_output.stderr, None);
            return Err(anyhow::anyhow!(
                "git clone failed with token and without token. token_error='{}', fallback_error='{}'",
                primary_error.trim(),
                fallback_error.trim()
            ));
        }

        Err(anyhow::anyhow!(
            "git clone failed: {}",
            primary_error.trim()
        ))
    }

    async fn run_git_clone(
        clone_url: &str,
        checkout_dir: &Path,
        token: Option<&str>,
    ) -> Result<std::process::Output> {
        let mut cmd = Command::new("git");
        cmd.env("GIT_TERMINAL_PROMPT", "0");

        if let Some(auth_token) = token {
            cmd.arg("-c").arg(format!(
                "http.extraheader=Authorization: Bearer {}",
                auth_token
            ));
        }

        cmd.arg("clone")
            .arg("--depth")
            .arg("1")
            .arg("--quiet")
            .arg(clone_url)
            .arg(checkout_dir);

        Ok(cmd.output().await?)
    }

    fn build_clone_url(owner: &str, repo: &str, repo_url: &str) -> String {
        let fallback = format!("https://github.com/{}/{}.git", owner, repo);

        let Ok(mut parsed) = Url::parse(repo_url) else {
            return fallback;
        };

        if !matches!(parsed.scheme(), "http" | "https") {
            return fallback;
        }

        if parsed.host_str().is_none() {
            return fallback;
        }

        parsed.set_query(None);
        parsed.set_fragment(None);

        let mut path = parsed.path().trim_end_matches('/').to_string();
        if path.is_empty() || path == "/" {
            path = format!("/{}/{}", owner, repo);
        }

        if !path.ends_with(".git") {
            path.push_str(".git");
        }

        parsed.set_path(&path);
        parsed.to_string()
    }

    fn should_retry_without_token(stderr: &str) -> bool {
        let lower = stderr.to_ascii_lowercase();
        lower.contains("invalid credentials")
            || lower.contains("authentication failed")
            || lower.contains("http basic: access denied")
            || lower.contains("could not read username")
            || lower.contains("repository not found")
    }

    fn sanitize_git_stderr(stderr: &[u8], token: Option<&str>) -> String {
        let mut text = String::from_utf8_lossy(stderr).to_string();
        if let Some(auth_token) = token {
            text = text.replace(auth_token, "***");
        }
        text
    }
}

fn collect_repository_files(repo_dir: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();

    for entry in WalkDir::new(repo_dir).follow_links(false) {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type().is_dir() {
            continue;
        }

        if path
            .components()
            .any(|c| c.as_os_str().to_string_lossy() == ".git")
        {
            continue;
        }

        let rel = path
            .strip_prefix(repo_dir)
            .map_err(|e| anyhow::anyhow!("failed to strip repo root: {}", e))?;
        let rel = rel.to_string_lossy().replace('\\', "/");
        if rel.is_empty() {
            continue;
        }

        files.insert(rel, std::fs::read(path)?);
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::{collect_repository_files, GithubClient};
    use std::fs;
    use std::io::Write;

    #[test]
    fn build_clone_url_uses_repository_host() {
        let url = GithubClient::build_clone_url(
            "acme",
            "skill-registry",
            "https://ghe.example.com/acme/skill-registry",
        );
        assert_eq!(url, "https://ghe.example.com/acme/skill-registry.git");
    }

    #[test]
    fn build_clone_url_falls_back_when_repo_url_invalid() {
        let url = GithubClient::build_clone_url("acme", "skill-registry", "not-a-url");
        assert_eq!(url, "https://github.com/acme/skill-registry.git");
    }

    #[test]
    fn build_clone_url_strips_query_and_fragment() {
        let url = GithubClient::build_clone_url(
            "acme",
            "skill-registry",
            "https://ghe.example.com/acme/skill-registry?ref=main#readme",
        );
        assert_eq!(url, "https://ghe.example.com/acme/skill-registry.git");
    }

    #[test]
    fn build_clone_url_rejects_unsupported_schemes_and_empty_paths() {
        let ssh = GithubClient::build_clone_url(
            "acme",
            "skill-registry",
            "ssh://ghe.example.com/acme/skill-registry",
        );
        assert_eq!(ssh, "https://github.com/acme/skill-registry.git");

        let no_path =
            GithubClient::build_clone_url("acme", "skill-registry", "https://ghe.example.com");
        assert_eq!(no_path, "https://ghe.example.com/acme/skill-registry.git");
    }

    #[test]
    fn should_retry_without_token_matches_auth_failures() {
        assert!(GithubClient::should_retry_without_token(
            "invalid credentials"
        ));
        assert!(GithubClient::should_retry_without_token(
            "HTTP Basic: Access denied"
        ));
        assert!(!GithubClient::should_retry_without_token("network timeout"));
    }

    #[test]
    fn sanitize_git_stderr_redacts_token() {
        let raw = b"fatal: token abc123 rejected";
        let sanitized = GithubClient::sanitize_git_stderr(raw, Some("abc123"));
        assert!(!sanitized.contains("abc123"));
        assert!(sanitized.contains("***"));
    }

    #[test]
    fn collect_repository_files_skips_git_directory() {
        let temp = tempfile::tempdir().unwrap();
        let repo_dir = temp.path();
        fs::create_dir_all(repo_dir.join("src")).unwrap();
        fs::create_dir_all(repo_dir.join(".git")).unwrap();

        let mut file = fs::File::create(repo_dir.join("src/lib.rs")).unwrap();
        writeln!(file, "fn main() {{}}").unwrap();

        let mut git_file = fs::File::create(repo_dir.join(".git/config")).unwrap();
        writeln!(git_file, "[core]").unwrap();

        let files = collect_repository_files(repo_dir).unwrap();
        assert!(files.contains_key("src/lib.rs"));
        assert!(!files.contains_key(".git/config"));
    }
}
