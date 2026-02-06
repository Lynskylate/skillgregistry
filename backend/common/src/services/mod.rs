pub mod discovery_registries;
pub mod github;
pub mod plugins;
pub mod registry;
pub mod skills;

use sea_orm::DbErr;

#[derive(Debug)]
pub struct ServiceError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (code: {})", self.message, self.code)
    }
}

impl std::error::Error for ServiceError {}

impl ServiceError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

impl From<DbErr> for ServiceError {
    fn from(err: DbErr) -> Self {
        Self::new(500, format!("Database error: {}", err))
    }
}
