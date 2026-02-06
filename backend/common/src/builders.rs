use crate::github::GithubClient;
use crate::repositories::{
    discovery_registries::DiscoveryRegistryRepositoryImpl, plugins::PluginRepositoryImpl,
    registry::RegistryRepositoryImpl, skills::SkillRepositoryImpl,
};
use crate::s3::S3Service;
use crate::services::{
    discovery_registries::DiscoveryRegistryServiceImpl, github::GithubService,
    plugins::PluginServiceImpl, registry::RegistryServiceImpl, skills::SkillServiceImpl,
};
use crate::settings::{S3Settings, Settings};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[derive(Clone)]
pub struct Repositories {
    pub skill_repo: Arc<dyn crate::repositories::skills::SkillRepository>,
    pub plugin_repo: Arc<dyn crate::repositories::plugins::PluginRepository>,
    pub registry_repo: Arc<dyn crate::repositories::registry::RegistryRepository>,
    pub discovery_registry_repo:
        Arc<dyn crate::repositories::discovery_registries::DiscoveryRegistryRepository>,
}

#[derive(Clone)]
pub struct Services {
    pub skill_service: Arc<dyn crate::services::skills::SkillService>,
    pub plugin_service: Arc<dyn crate::services::plugins::PluginService>,
    pub registry_service: Arc<dyn crate::services::registry::RegistryService>,
    pub discovery_registry_service:
        Arc<dyn crate::services::discovery_registries::DiscoveryRegistryService>,
    pub github_service: Arc<dyn GithubService>,
    pub s3: Arc<S3Service>,
}

pub fn build_repositories(db: Arc<DatabaseConnection>) -> Repositories {
    Repositories {
        skill_repo: Arc::new(SkillRepositoryImpl::new(db.clone())),
        plugin_repo: Arc::new(PluginRepositoryImpl::new(db.clone())),
        registry_repo: Arc::new(RegistryRepositoryImpl::new(db.clone())),
        discovery_registry_repo: Arc::new(DiscoveryRegistryRepositoryImpl::new(db.clone())),
    }
}

pub fn build_github_service(token: Option<String>, api_url: String) -> Arc<dyn GithubService> {
    Arc::new(GithubClient::new(token, api_url))
}

pub async fn build_services(repos: &Repositories, settings: &Settings) -> Services {
    let skill_service = Arc::new(SkillServiceImpl::new(
        repos.skill_repo.clone(),
        repos.registry_repo.clone(),
    ));

    let plugin_service = Arc::new(PluginServiceImpl::new(
        repos.plugin_repo.clone(),
        repos.registry_repo.clone(),
        repos.skill_repo.clone(),
    ));

    let registry_service = Arc::new(RegistryServiceImpl::new(repos.registry_repo.clone()));

    let discovery_registry_service = Arc::new(DiscoveryRegistryServiceImpl::new(
        repos.discovery_registry_repo.clone(),
    ));

    let github_service = build_github_service(
        settings.github.token.clone(),
        settings.github.api_url.clone(),
    );

    let s3 = build_s3_service(&settings.s3).await;

    Services {
        skill_service,
        plugin_service,
        registry_service,
        discovery_registry_service,
        github_service,
        s3,
    }
}

pub async fn build_s3_service(s3_settings: &S3Settings) -> Arc<S3Service> {
    Arc::new(
        S3Service::new(
            s3_settings.bucket.clone(),
            s3_settings.region.clone(),
            s3_settings.endpoint.clone(),
            s3_settings.access_key_id.clone(),
            s3_settings.secret_access_key.clone(),
            s3_settings.force_path_style,
        )
        .await,
    )
}

pub async fn build_all(
    db: Arc<DatabaseConnection>,
    settings: &Settings,
) -> (Repositories, Services) {
    let repos = build_repositories(db.clone());
    let services = build_services(&repos, settings).await;
    (repos, services)
}
