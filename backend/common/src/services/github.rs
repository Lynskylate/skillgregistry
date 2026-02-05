use crate::github::{GithubClient, GithubRepo};

#[async_trait::async_trait]
pub trait GithubService: Send + Sync {
    async fn search_repositories(&self, query: &str) -> anyhow::Result<Vec<GithubRepo>>;
    async fn search_code(&self, query: &str) -> anyhow::Result<Vec<GithubRepo>>;
    async fn download_zipball(&self, owner: &str, repo: &str) -> anyhow::Result<Vec<u8>>;
}

#[async_trait::async_trait]
impl GithubService for GithubClient {
    async fn search_repositories(&self, query: &str) -> anyhow::Result<Vec<GithubRepo>> {
        self.search_repositories(query).await
    }

    async fn search_code(&self, query: &str) -> anyhow::Result<Vec<GithubRepo>> {
        self.search_code(query).await
    }

    async fn download_zipball(&self, owner: &str, repo: &str) -> anyhow::Result<Vec<u8>> {
        self.download_zipball(owner, repo).await
    }
}
