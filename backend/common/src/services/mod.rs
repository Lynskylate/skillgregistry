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

#[cfg(test)]
mod tests {
    use super::ServiceError;
    use sea_orm::DbErr;

    #[test]
    fn service_error_new_sets_fields() {
        let err = ServiceError::new(404, "not found");

        assert_eq!(err.code, 404);
        assert_eq!(err.message, "not found");
        assert!(err.data.is_none());
    }

    #[test]
    fn service_error_with_data_attaches_payload() {
        let err = ServiceError::new(422, "validation failed")
            .with_data(serde_json::json!({"field": "email"}));

        assert_eq!(err.code, 422);
        assert_eq!(err.message, "validation failed");
        assert_eq!(err.data, Some(serde_json::json!({"field": "email"})));
    }

    #[test]
    fn from_db_err_preserves_message() {
        let err = ServiceError::from(DbErr::Custom("boom".to_string()));

        assert_eq!(err.code, 500);
        assert!(err.message.contains("Database error"));
        assert!(err.message.contains("boom"));
        assert!(err.to_string().contains("code: 500"));
    }
}
