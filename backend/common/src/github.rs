use crate::infra::github_http::{build_github_client, send_request_with_retry};
use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;

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
            let resp = send_request_with_retry(req, "search repositories").await?;
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
            let resp = send_request_with_retry(req, "search code").await?;
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
        let resp = send_request_with_retry(req, "download zipball").await?;
        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }
}
