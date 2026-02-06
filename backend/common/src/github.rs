use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::{Client, Response, StatusCode};
use serde::Deserialize;
use std::time::Duration;

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
    pub stargazers_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner: GithubOwner,
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
            let url = format!("{}/search/repositories", self.api_url);
            tracing::debug!("Fetching repo search page {}", page);

            let per_page_str = per_page.to_string();
            let page_str = page.to_string();

            let resp = self
                .send_request_with_retry(self.client.get(&url).query(&[
                    ("q", query),
                    ("per_page", per_page_str.as_str()),
                    ("page", page_str.as_str()),
                ]))
                .await?;
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
            tracing::debug!("Fetching code search page {}", page);

            let per_page_str = per_page.to_string();
            let page_str = page.to_string();

            let resp = self
                .send_request_with_retry(self.client.get(&url).query(&[
                    ("q", query),
                    ("per_page", per_page_str.as_str()),
                    ("page", page_str.as_str()),
                ]))
                .await?;
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
        let resp = self.send_request_with_retry(self.client.get(&url)).await?;
        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn send_request_with_retry(&self, req: reqwest::RequestBuilder) -> Result<Response> {
        let mut attempts = 0;
        loop {
            attempts += 1;
            let resp = req
                .try_clone()
                .ok_or_else(|| anyhow::anyhow!("failed to clone request"))?
                .send()
                .await?;

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
                    return Err(anyhow::anyhow!(
                        "Unprocessable Entity (422) when calling GitHub API. Check query syntax."
                    ));
                }
                status => {
                    if attempts >= 3 {
                        return Err(anyhow::anyhow!("Request failed with status: {}", status));
                    }
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempts))).await;
                }
            }
        }
    }
}
