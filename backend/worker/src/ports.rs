use anyhow::Result;
use async_trait::async_trait;
#[cfg(test)]
use mockall::automock;
use std::collections::BTreeMap;

use crate::github::{GithubClient, GithubRepo};
use common::s3::S3Service;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait GithubApi: Send + Sync {
    async fn search_repositories(&self, query: &str) -> Result<Vec<GithubRepo>>;
    async fn search_code(&self, query: &str) -> Result<Vec<GithubRepo>>;
    async fn clone_repository_files(
        &self,
        owner: &str,
        repo: &str,
        repo_url: &str,
        token: Option<String>,
    ) -> Result<BTreeMap<String, Vec<u8>>>;
}

#[async_trait]
impl GithubApi for GithubClient {
    async fn search_repositories(&self, query: &str) -> Result<Vec<GithubRepo>> {
        self.search_repositories(query).await
    }

    async fn search_code(&self, query: &str) -> Result<Vec<GithubRepo>> {
        self.search_code(query).await
    }

    async fn clone_repository_files(
        &self,
        owner: &str,
        repo: &str,
        repo_url: &str,
        token: Option<String>,
    ) -> Result<BTreeMap<String, Vec<u8>>> {
        self.clone_repository_files(owner, repo, repo_url, token)
            .await
    }
}

#[cfg_attr(test, automock)]
#[async_trait]
pub trait Storage: Send + Sync {
    async fn upload(&self, key: &str, body: Vec<u8>) -> Result<String>;
    async fn download(&self, key: &str) -> Result<Vec<u8>>;
}

#[async_trait]
impl Storage for S3Service {
    async fn upload(&self, key: &str, body: Vec<u8>) -> Result<String> {
        self.upload_file(key, body).await
    }

    async fn download(&self, key: &str) -> Result<Vec<u8>> {
        self.download_file(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn github_api_impl_forwards_errors() {
        let client = GithubClient::new(None, "http://127.0.0.1:1".to_string()).unwrap();
        let api: &dyn GithubApi = &client;

        assert!(api.search_repositories("topic:agent-skill").await.is_err());
        assert!(api.search_code("path:SKILL.md").await.is_err());
        assert!(api
            .clone_repository_files("acme", "skills", "https://github.com/acme/skills", None,)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn storage_impl_forwards_errors() {
        let s3 = common::s3::S3Service::new(
            "skills".to_string(),
            "us-east-1".to_string(),
            Some("http://127.0.0.1:1".to_string()),
            None,
            None,
            true,
        )
        .await;
        let storage: &dyn Storage = &s3;

        assert!(storage.download("missing").await.is_err());
        assert!(storage.upload("artifact.zip", vec![1, 2, 3]).await.is_err());
    }
}
