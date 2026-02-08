use crate::activities::{discovery::DiscoveryActivities, sync::SyncActivities};
use crate::github;
use crate::sync::SyncService;
use anyhow::Result;
use common::build_all;
use common::settings::Settings;
use common::{Repositories, Services};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub struct WorkerContext {
    pub db: Arc<DatabaseConnection>,
    pub repos: Repositories,
    pub services: Services,
    pub github: Arc<github::GithubClient>,
    pub settings: Arc<Settings>,
}

impl std::fmt::Debug for WorkerContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerContext")
            .field("db", &self.db)
            .field("settings", &self.settings)
            .field("s3", &"S3Service")
            .field("github", &"GithubClient")
            .finish()
    }
}

pub struct WorkerServices {
    pub discovery: Arc<DiscoveryActivities>,
    pub sync: Arc<SyncActivities>,
}

pub async fn build_worker_context(settings: Settings) -> Result<Arc<WorkerContext>> {
    let db = common::db::establish_connection(&settings.database.url).await?;
    let db = Arc::new(db);
    let (repos, services) = build_all(db.clone(), &settings).await?;

    let github = Arc::new(github::GithubClient::new(
        settings.github.token.clone(),
        settings.github.api_url.clone(),
    )?);

    Ok(Arc::new(WorkerContext {
        db,
        repos,
        services,
        github,
        settings: Arc::new(settings),
    }))
}

pub fn build_worker_services(ctx: &Arc<WorkerContext>) -> WorkerServices {
    let sync_service = Arc::new(SyncService::new(
        (*ctx.db).clone(),
        ctx.services.s3.clone(),
        ctx.github.clone(),
        ctx.services.registry_service.clone(),
        ctx.services.discovery_registry_service.clone(),
    ));

    let discovery = Arc::new(
        DiscoveryActivities::new(ctx.db.clone(), ctx.github.clone())
            .with_registry_service(ctx.services.discovery_registry_service.clone()),
    );

    let sync = Arc::new(SyncActivities::new(sync_service));

    WorkerServices { discovery, sync }
}
