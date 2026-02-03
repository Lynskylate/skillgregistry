use anyhow::Result;
use async_trait::async_trait;
#[cfg(test)]
use mockall::automock;

use crate::github::{GithubClient, GithubRepo};
use common::s3::S3Service;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait GithubApi: Send + Sync {
    async fn search_repositories(&self, query: &str) -> Result<Vec<GithubRepo>>;
    async fn search_code(&self, query: &str) -> Result<Vec<GithubRepo>>;
    async fn download_zipball(&self, owner: &str, repo: &str) -> Result<Vec<u8>>;
}

#[async_trait]
impl GithubApi for GithubClient {
    async fn search_repositories(&self, query: &str) -> Result<Vec<GithubRepo>> {
        self.search_repositories(query).await
    }

    async fn search_code(&self, query: &str) -> Result<Vec<GithubRepo>> {
        self.search_code(query).await
    }

    async fn download_zipball(&self, owner: &str, repo: &str) -> Result<Vec<u8>> {
        self.download_zipball(owner, repo).await
    }
}

#[cfg_attr(test, automock)]
#[async_trait]
pub trait Storage: Send + Sync {
    async fn upload(&self, key: &str, body: Vec<u8>) -> Result<String>;
}

#[async_trait]
impl Storage for S3Service {
    async fn upload(&self, key: &str, body: Vec<u8>) -> Result<String> {
        self.upload_file(key, body).await
    }
}
