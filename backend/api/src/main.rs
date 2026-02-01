mod handlers;
mod models;

use axum::{
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use common::db;
use common::config::AppConfig;
use sea_orm::DatabaseConnection;
use tower_http::cors::CorsLayer;

pub struct AppState {
    pub db: DatabaseConnection,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if dotenv::dotenv().is_err() {
        if let Ok(cwd) = std::env::current_dir() {
            let candidates = [
                cwd.join(".env"),
                cwd.join("../.env"),
                cwd.join("../../.env"),
                cwd.join("../../../.env"),
            ];
            for p in candidates {
                if p.exists() && dotenv::from_path(&p).is_ok() {
                    break;
                }
            }
        }
    }

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "api=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::new().expect("Failed to load configuration");

    let db_url = std::env::var("DATABASE_URL").ok()
        .or(config.database.as_ref().and_then(|d| d.url.clone()))
        .unwrap_or_else(|| "sqlite://skillregistry.db?mode=rwc".to_string());
        
    let db = db::establish_connection(&db_url).await?;

    let state = Arc::new(AppState { db });

    let app = Router::new()
        .route("/", get(|| async { "Skill Registry API" }))
        .route("/api/skills", get(handlers::list_skills))
        .route("/api/skills/:owner/:repo/:name", get(handlers::get_skill))
        .route("/api/skills/:owner/:repo/:name/versions/:version", get(handlers::get_skill_version))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
