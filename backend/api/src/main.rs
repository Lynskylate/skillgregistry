mod auth;
mod handlers;
mod models;

use axum::{http::HeaderValue, routing::get, Router};
use common::db;
use common::settings::Settings;
use sea_orm::DatabaseConnection;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct AppState {
    pub db: DatabaseConnection,
    pub settings: Settings,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::new().expect("Failed to load configuration");

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "api=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_url = settings.database.url.clone();

    let db = db::establish_connection(&db_url).await?;

    let state = Arc::new(AppState {
        db,
        settings: settings.clone(),
    });

    let cors = build_cors(&settings);

    let app = Router::new()
        .route("/", get(|| async { "Skill Registry API" }))
        .route("/api/skills", get(handlers::list_skills))
        .route("/api/skills/:owner/:repo/:name", get(handlers::get_skill))
        .route(
            "/api/skills/:owner/:repo/:name/versions/:version",
            get(handlers::get_skill_version),
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
        .nest("/api/auth", auth::router())
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], settings.port));
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

fn build_cors(settings: &Settings) -> CorsLayer {
    let origin = settings
        .auth
        .frontend_origin
        .as_ref()
        .and_then(|s| HeaderValue::from_str(s).ok());

    match (settings.debug, origin) {
        (false, Some(origin)) => CorsLayer::new()
            .allow_origin(origin)
            .allow_credentials(true)
            .allow_headers([
                axum::http::header::CONTENT_TYPE,
                axum::http::header::AUTHORIZATION,
            ])
            .allow_methods(Any),
        _ => CorsLayer::permissive(),
    }
}
