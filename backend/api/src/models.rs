use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: String,
    pub data: Option<T>,
    pub timestamp: i64,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            code: 200,
            message: "Success".to_string(),
            data: Some(data),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn error(code: i32, message: String) -> Self {
        Self {
            code,
            message,
            data: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}
