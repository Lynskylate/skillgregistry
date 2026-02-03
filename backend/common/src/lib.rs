pub mod db;
pub mod entities;
pub mod s3;
pub mod settings;

use sea_orm::DatabaseConnection;

pub struct AppState {
    pub db: DatabaseConnection,
}
