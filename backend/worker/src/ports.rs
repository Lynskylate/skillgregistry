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
        token: Option<String>,
    ) -> Result<BTreeMap<String, Vec<u8>>> {
        self.clone_repository_files(owner, repo, token).await
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
