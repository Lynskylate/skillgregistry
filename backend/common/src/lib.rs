pub mod config;
pub mod db;
pub mod entities;
pub mod s3;

use sea_orm::DatabaseConnection;

pub struct AppState {
    pub db: DatabaseConnection,
}
