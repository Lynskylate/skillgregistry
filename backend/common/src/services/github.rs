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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn github_service_impl_forwards_errors_from_client() {
        let client = GithubClient::new(None, "http://127.0.0.1:1".to_string()).unwrap();
        let service: &dyn GithubService = &client;

        assert!(service
            .search_repositories("topic:agent-skill")
            .await
            .is_err());
        assert!(service.search_code("path:SKILL.md").await.is_err());
        assert!(service.download_zipball("acme", "repo").await.is_err());
    }
}
