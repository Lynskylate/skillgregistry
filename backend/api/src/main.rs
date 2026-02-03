mod handlers;
mod models;

use axum::{routing::get, Router};
use common::db;
use common::settings::Settings;
use sea_orm::DatabaseConnection;
use std::net::SocketAddr;
use std::sync::Arc;
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

    let app = Router::new()
        .route("/", get(|| async { "Skill Registry API" }))
        .route("/api/skills", get(handlers::list_skills))
        .route("/api/skills/:owner/:repo/:name", get(handlers::get_skill))
        .route(
            "/api/skills/:owner/:repo/:name/versions/:version",
            get(handlers::get_skill_version),
        )
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], settings.port));
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
