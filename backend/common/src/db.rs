use crate::entities::prelude::*;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr, Schema};

pub async fn establish_connection(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    let db = Database::connect(database_url).await?;

    // Auto-migration logic
    let builder = db.get_database_backend();
    let schema = Schema::new(builder);

    // Create tables if not exist
    match db
        .execute(schema.create_table_from_entity(Users).if_not_exists())
        .await
    {
        Ok(_) => tracing::info!("Ensured table users exists"),
        Err(e) => tracing::warn!("Failed to create table users: {}", e),
    }

    match db
        .execute(
            schema
                .create_table_from_entity(AuthIdentities)
                .if_not_exists(),
        )
        .await
    {
        Ok(_) => tracing::info!("Ensured table auth_identities exists"),
        Err(e) => tracing::warn!("Failed to create table auth_identities: {}", e),
    }

    match db
        .execute(
            schema
                .create_table_from_entity(LocalCredentials)
                .if_not_exists(),
        )
        .await
    {
        Ok(_) => tracing::info!("Ensured table local_credentials exists"),
        Err(e) => tracing::warn!("Failed to create table local_credentials: {}", e),
    }

    match db
        .execute(
            schema
                .create_table_from_entity(RefreshTokens)
                .if_not_exists(),
        )
        .await
    {
        Ok(_) => tracing::info!("Ensured table refresh_tokens exists"),
        Err(e) => tracing::warn!("Failed to create table refresh_tokens: {}", e),
    }

    match db
        .execute(
            schema
                .create_table_from_entity(Organizations)
                .if_not_exists(),
        )
        .await
    {
        Ok(_) => tracing::info!("Ensured table organizations exists"),
        Err(e) => tracing::warn!("Failed to create table organizations: {}", e),
    }

    match db
        .execute(
            schema
                .create_table_from_entity(OrgMemberships)
                .if_not_exists(),
        )
        .await
    {
        Ok(_) => tracing::info!("Ensured table org_memberships exists"),
        Err(e) => tracing::warn!("Failed to create table org_memberships: {}", e),
    }

    match db
        .execute(
            schema
                .create_table_from_entity(SsoConnections)
                .if_not_exists(),
        )
        .await
    {
        Ok(_) => tracing::info!("Ensured table sso_connections exists"),
        Err(e) => tracing::warn!("Failed to create table sso_connections: {}", e),
    }

    match db
        .execute(
            schema
                .create_table_from_entity(SsoIdentities)
                .if_not_exists(),
        )
        .await
    {
        Ok(_) => tracing::info!("Ensured table sso_identities exists"),
        Err(e) => tracing::warn!("Failed to create table sso_identities: {}", e),
    }

    match db
        .execute(
            schema
                .create_table_from_entity(SkillRegistry)
                .if_not_exists(),
        )
        .await
    {
        Ok(_) => tracing::info!("Ensured table skill_registry exists"),
        Err(e) => tracing::warn!("Failed to create table skill_registry: {}", e),
    }

    match db
        .execute(schema.create_table_from_entity(Skills).if_not_exists())
        .await
    {
        Ok(_) => tracing::info!("Ensured table skills exists"),
        Err(e) => tracing::warn!("Failed to create table skills: {}", e),
    }

    match db
        .execute(
            schema
                .create_table_from_entity(SkillVersions)
                .if_not_exists(),
        )
        .await
    {
        Ok(_) => tracing::info!("Ensured table skill_versions exists"),
        Err(e) => tracing::warn!("Failed to create table skill_versions: {}", e),
    }

    match db
        .execute(schema.create_table_from_entity(TaskLogs).if_not_exists())
        .await
    {
        Ok(_) => tracing::info!("Ensured table task_logs exists"),
        Err(e) => tracing::warn!("Failed to create table task_logs: {}", e),
    }

    match db
        .execute(schema.create_table_from_entity(Blacklist).if_not_exists())
        .await
    {
        Ok(_) => tracing::info!("Ensured table blacklist exists"),
        Err(e) => tracing::warn!("Failed to create table blacklist: {}", e),
    }

    Ok(db)
}
