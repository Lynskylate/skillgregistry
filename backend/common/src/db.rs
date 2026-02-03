use crate::entities::prelude::*;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr, Schema};

pub async fn establish_connection(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    let db = Database::connect(database_url).await?;

    // Auto-migration logic
    let builder = db.get_database_backend();
    let schema = Schema::new(builder);

    // Create tables if not exist
    let stmt = builder.build(
        schema
            .create_table_from_entity(SkillRegistry)
            .if_not_exists(),
    );
    match db.execute(stmt).await {
        Ok(_) => tracing::info!("Ensured table skill_registry exists"),
        Err(e) => tracing::warn!("Failed to create table skill_registry: {}", e),
    }

    let stmt = builder.build(schema.create_table_from_entity(Skills).if_not_exists());
    match db.execute(stmt).await {
        Ok(_) => tracing::info!("Ensured table skills exists"),
        Err(e) => tracing::warn!("Failed to create table skills: {}", e),
    }

    let stmt = builder.build(
        schema
            .create_table_from_entity(SkillVersions)
            .if_not_exists(),
    );
    match db.execute(stmt).await {
        Ok(_) => tracing::info!("Ensured table skill_versions exists"),
        Err(e) => tracing::warn!("Failed to create table skill_versions: {}", e),
    }

    let stmt = builder.build(schema.create_table_from_entity(TaskLogs).if_not_exists());
    match db.execute(stmt).await {
        Ok(_) => tracing::info!("Ensured table task_logs exists"),
        Err(e) => tracing::warn!("Failed to create table task_logs: {}", e),
    }

    let stmt = builder.build(schema.create_table_from_entity(Blacklist).if_not_exists());
    match db.execute(stmt).await {
        Ok(_) => tracing::info!("Ensured table blacklist exists"),
        Err(e) => tracing::warn!("Failed to create table blacklist: {}", e),
    }

    Ok(db)
}
