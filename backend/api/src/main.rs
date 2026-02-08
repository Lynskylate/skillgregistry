mod auth;
mod handlers;
mod models;
mod origin;

use axum::{
    http::{request::Parts, HeaderValue},
    routing::{get, patch, post},
    Router,
};
use common::build_all;
use common::settings::Settings;
use origin::{is_origin_allowed, parse_frontend_origins};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::AllowOrigin;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<sea_orm::DatabaseConnection>,
    pub settings: Settings,
    pub services: common::Services,
    pub repos: common::Repositories,
    pub allowed_frontend_origins: Arc<Vec<String>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::new()?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "api=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_url = settings.database.url.clone();

    let db = common::db::establish_connection(&db_url).await?;
    let db = Arc::new(db);

    let (repos, services) = build_all(db.clone(), &settings).await?;

    let allowed_frontend_origins = Arc::new(parse_frontend_origins(
        settings.auth.frontend_origin.as_deref(),
    ));

    let state = Arc::new(AppState {
        db,
        settings: settings.clone(),
        services,
        repos,
        allowed_frontend_origins: Arc::clone(&allowed_frontend_origins),
    });

    let cors = build_cors(settings.debug, Arc::clone(&allowed_frontend_origins));

    let app: Router = Router::new()
        .route("/", get(|| async { "Skill Registry API" }))
        .route("/api/skills", get(handlers::list_skills))
        .route(
            "/api/:host/:org/:repo/skill/:name",
            get(handlers::get_repo_skill_detail),
        )
        .route(
            "/api/:host/:org/:repo/skill/:name/versions/:version",
            get(handlers::get_repo_skill_version),
        )
        .route(
            "/api/:host/:org/:repo/skill/:name/download",
            get(handlers::download_repo_skill),
        )
        .route(
            "/api/:host/:org/:repo/plugin",
            get(handlers::list_repo_plugins),
        )
        .route(
            "/api/:host/:org/:repo/plugin/:plugin_name",
            get(handlers::get_repo_plugin),
        )
        .route(
            "/api/:host/:org/:repo/plugin/:plugin_name/agent/:agent_name",
            get(handlers::get_repo_plugin_agent),
        )
        .route(
            "/api/:host/:org/:repo/plugin/:plugin_name/skill/:skill_name",
            get(handlers::get_repo_plugin_skill),
        )
        .route(
            "/api/:host/:org/:repo/plugin/:plugin_name/command/:command_name",
            get(handlers::get_repo_plugin_command),
        )
        .route(
            "/api/:host/:org/:repo/skill",
            get(handlers::list_repo_skills),
        )
        .route("/api/me", get(auth::me))
        .route(
            "/api/admin/discovery-registries",
            get(handlers::list_discovery_registries).post(handlers::create_discovery_registry),
        )
        .route(
            "/api/admin/discovery-registries/:id",
            patch(handlers::update_discovery_registry).delete(handlers::delete_discovery_registry),
        )
        .route(
            "/api/admin/discovery-registries/:id/validate-delete",
            post(handlers::validate_delete_discovery_registry),
        )
        .route(
            "/api/admin/discovery-registries/:id/test-health",
            post(handlers::test_discovery_registry_health),
        )
        .route(
            "/api/admin/discovery-registries/:id/trigger",
            post(handlers::trigger_discovery_registry),
        )
        .nest("/api/auth", auth::router())
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], settings.port));
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn build_cors(debug: bool, allowed_origins: Arc<Vec<String>>) -> CorsLayer {
    match (debug, allowed_origins.is_empty()) {
        (false, false) => {
            let allow_origin =
                AllowOrigin::predicate(move |origin: &HeaderValue, _request: &Parts| {
                    let Ok(origin) = origin.to_str() else {
                        return false;
                    };
                    is_origin_allowed(allowed_origins.as_slice(), origin)
                });

            CorsLayer::new()
                .allow_origin(allow_origin)
                .allow_credentials(true)
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                ])
                .allow_methods(Any)
        }
        _ => CorsLayer::permissive(),
    }
}
