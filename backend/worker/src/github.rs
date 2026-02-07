use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::{Client, Response, StatusCode};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;
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

#[derive(Deserialize, Debug)]
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

#[derive(Deserialize, Debug)]
pub struct GithubOwner {
    pub login: String,
}

pub struct GithubClient {
    client: Client,
    api_url: String,
}

impl GithubClient {
    pub fn new(token: Option<String>, api_url: String) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("User-Agent", "SkillRegistry/1.0".parse().unwrap());
        headers.insert("Accept", "application/vnd.github.v3+json".parse().unwrap());

        if let Some(ref t) = token {
            headers.insert("Authorization", format!("Bearer {}", t).parse().unwrap());
        }

        Self {
            client: Client::builder().default_headers(headers).build().unwrap(),
            api_url,
        }
    }

    pub async fn search_repositories(&self, query: &str) -> Result<Vec<GithubRepo>> {
        let mut all_repos = Vec::new();
        let mut page = 1;
        let per_page = 100;

        loop {
            let url = format!(
                "{}/search/repositories?q={}&per_page={}&page={}",
                self.api_url, query, per_page, page
            );
            tracing::debug!("Fetching page {}: {}", page, url);

            let resp = self.send_request_with_retry(&url).await?;
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

        // Code search rate limits are stricter, and results per page max is 100.
        // Also total results are limited to 1000.
        loop {
            let url = format!(
                "{}/search/code?q={}&per_page={}&page={}",
                self.api_url, query, per_page, page
            );
            tracing::debug!("Fetching code page {}: {}", page, url);

            let resp = self.send_request_with_retry(&url).await?;
            let search_resp: GithubCodeSearchResponse = resp.json().await?;

            if search_resp.items.is_empty() {
                break;
            }

            // Code search returns items with minimal repo info.
            // The `repository` field in GithubCodeItem contains the GithubRepo structure
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
        let resp = self.send_request_with_retry(&url).await?;
        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }

    pub async fn clone_repository_files(
        &self,
        owner: &str,
        repo: &str,
        token: Option<String>,
    ) -> Result<BTreeMap<String, Vec<u8>>> {
        let temp_dir = tempfile::tempdir()?;
        let checkout_dir = temp_dir.path().join("repo");
        let clone_url = format!("https://github.com/{}/{}.git", owner, repo);

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

    async fn send_request_with_retry(&self, url: &str) -> Result<Response> {
        let mut attempts = 0;
        loop {
            attempts += 1;
            let resp = self.client.get(url).send().await?;

            match resp.status() {
                StatusCode::OK => return Ok(resp),
                StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => {
                    if attempts >= 5 {
                        return Err(anyhow::anyhow!(
                            "Rate limit exceeded after {} attempts",
                            attempts
                        ));
                    }

                    let wait_time = if let Some(retry_after) = resp.headers().get("Retry-After") {
                        retry_after.to_str().unwrap_or("60").parse().unwrap_or(60)
                    } else {
                        60
                    };

                    tracing::warn!("Rate limit hit, waiting {}s...", wait_time);
                    tokio::time::sleep(Duration::from_secs(wait_time)).await;
                }
                StatusCode::UNPROCESSABLE_ENTITY => {
                    // 422 usually means validation failed, e.g. search query too long or specific constraints
                    return Err(anyhow::anyhow!(
                        "Unprocessable Entity (422) on url {}. Check query syntax.",
                        url
                    ));
                }
                _ => {
                    if attempts >= 3 {
                        return Err(anyhow::anyhow!(
                            "Request failed: {} on url {}",
                            resp.status(),
                            url
                        ));
                    }
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempts))).await;
                }
            }
        }
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
