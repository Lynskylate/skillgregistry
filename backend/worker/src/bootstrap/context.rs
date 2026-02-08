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

#[cfg(test)]
mod tests {
    use super::*;
    use migration::MigratorTrait;
    use sea_orm::Database;

    fn test_settings() -> Settings {
        Settings {
            port: 3000,
            database: common::settings::DatabaseSettings {
                url: "sqlite::memory:".to_string(),
            },
            s3: common::settings::S3Settings {
                bucket: "test".to_string(),
                region: "us-east-1".to_string(),
                endpoint: None,
                access_key_id: None,
                secret_access_key: None,
                force_path_style: false,
            },
            github: common::settings::GithubSettings {
                search_keywords: "topic:agent-skill".to_string(),
                token: None,
                api_url: "https://api.github.com".to_string(),
            },
            worker: common::settings::WorkerSettings {
                scan_interval_seconds: 3600,
            },
            temporal: common::settings::TemporalSettings {
                server_url: "http://localhost:7233".to_string(),
                task_queue: "test-q".to_string(),
            },
            auth: common::settings::AuthSettings::default(),
            debug: true,
        }
    }

    #[tokio::test]
    async fn worker_context_debug_and_services_build() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        migration::Migrator::up(&db, None).await.unwrap();

        let settings = test_settings();
        let db_arc = Arc::new(db);
        let (repos, services) = common::build_all(db_arc.clone(), &settings).await.unwrap();
        let github =
            Arc::new(github::GithubClient::new(None, settings.github.api_url.clone()).unwrap());

        let ctx = Arc::new(WorkerContext {
            db: db_arc,
            repos,
            services,
            github,
            settings: Arc::new(settings),
        });

        let dbg = format!("{:?}", ctx);
        assert!(dbg.contains("WorkerContext"));
        assert!(dbg.contains("S3Service"));

        let worker_services = build_worker_services(&ctx);
        assert!(Arc::strong_count(&worker_services.discovery) >= 1);
        assert!(Arc::strong_count(&worker_services.sync) >= 1);
    }
}
