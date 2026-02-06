pub mod builders;
pub mod config;
pub mod db;
pub mod domain;
pub mod entities;
pub mod github;
pub mod infra;
pub mod persistence;
pub mod repositories;
pub mod s3;
pub mod services;
pub mod settings;

use sea_orm::DatabaseConnection;

pub struct AppState {
    pub db: DatabaseConnection,
}

pub use builders::{
    build_all, build_repositories, build_s3_service, build_services, Repositories, Services,
};
pub use github::GithubClient;
pub use services::{plugins, registry, skills, ServiceError};
