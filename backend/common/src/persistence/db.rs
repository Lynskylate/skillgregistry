use sea_orm::{Database, DatabaseConnection, DbErr};

pub async fn establish_connection(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    Database::connect(database_url).await
}

#[cfg(test)]
mod tests {
    use super::establish_connection;

    #[tokio::test]
    async fn establish_connection_accepts_sqlite_memory_url() {
        let conn = establish_connection("sqlite::memory:").await;
        assert!(conn.is_ok());
    }

    #[tokio::test]
    async fn establish_connection_rejects_invalid_url() {
        let conn = establish_connection("not-a-valid-db-url").await;
        assert!(conn.is_err());
    }
}
