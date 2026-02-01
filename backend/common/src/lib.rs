pub mod entities;
pub mod db;
pub mod s3;
pub mod config;

use sea_orm::DatabaseConnection;

pub struct AppState {
    pub db: DatabaseConnection,
}
