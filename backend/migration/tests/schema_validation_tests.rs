//! Migration schema validation tests
//!
//! These tests ensure that the database schema after running migrations
//! matches the entity definitions in `common::entities`.

use migration::Migrator;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, EntityTrait, PaginatorTrait};
use sea_orm_migration::MigratorTrait;

async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test database");

    // Run all migrations
    Migrator::up(&db, None)
        .await
        .expect("Failed to run migrations");

    db
}

// Test that verifies all tables exist by querying them
#[tokio::test]
async fn test_all_tables_exist() {
    let db = setup_test_db().await;

    let expected_tables = vec![
        "skill_registry",
        "skills",
        "skill_versions",
        "blacklist",
        "users",
        "local_credentials",
        "refresh_tokens",
        "organizations",
        "org_memberships",
        "sso_connections",
        "sso_identities",
        "auth_identities",
        "plugins",
        "plugin_versions",
        "plugin_components",
        "task_logs",
    ];

    for table in expected_tables {
        // Try to query each table - this will fail if the table doesn't exist
        let sql = format!("SELECT 1 FROM {} LIMIT 1", table);
        let result: Result<sea_orm::ExecResult, sea_orm::DbErr> = db.execute_unprepared(&sql).await;
        assert!(
            result.is_ok(),
            "Expected table '{}' not found or not accessible: {:?}",
            table,
            result.err()
        );
    }
}

// Test that verifies entities can be used with the migrated database
// by inserting and selecting data
#[tokio::test]
async fn test_skill_registry_entity_matches_schema() {
    let db = setup_test_db().await;

    use common::entities::skill_registry::{self};
    use sea_orm::{ActiveModelTrait, Set};

    // Create a test entity
    let skill_reg = skill_registry::ActiveModel {
        platform: Set(common::entities::skill_registry::Platform::Github),
        owner: Set("test-owner".to_string()),
        name: Set("test-repo".to_string()),
        url: Set("https://github.com/test-owner/test-repo".to_string()),
        description: Set(Some("Test description".to_string())),
        repo_type: Set(None),
        status: Set("active".to_string()),
        blacklist_reason: Set(None),
        blacklisted_at: Set(None),
        stars: Set(0),
        last_scanned_at: Set(None),
        created_at: Set(chrono::Utc::now().naive_utc()),
        updated_at: Set(chrono::Utc::now().naive_utc()),
        ..Default::default()
    };

    // Insert should work if schema matches entity
    let result = skill_reg.insert(&db).await;
    assert!(
        result.is_ok(),
        "Failed to insert into skill_registry: {:?}",
        result.err()
    );

    // Verify we can query it back
    let count = skill_registry::Entity::find().count(&db).await;
    assert!(count.is_ok());
    assert_eq!(count.unwrap(), 1);
}

#[tokio::test]
async fn test_skills_entity_matches_schema() {
    let db = setup_test_db().await;

    use common::entities::skill_registry;
    use common::entities::skills;
    use sea_orm::{ActiveModelTrait, Set};

    // First create a skill_registry entry
    let skill_reg = skill_registry::ActiveModel {
        platform: Set(common::entities::skill_registry::Platform::Github),
        owner: Set("test-owner".to_string()),
        name: Set("test-repo".to_string()),
        url: Set("https://github.com/test-owner/test-repo".to_string()),
        description: Set(None),
        repo_type: Set(None),
        status: Set("active".to_string()),
        blacklist_reason: Set(None),
        blacklisted_at: Set(None),
        stars: Set(0),
        last_scanned_at: Set(None),
        created_at: Set(chrono::Utc::now().naive_utc()),
        updated_at: Set(chrono::Utc::now().naive_utc()),
        ..Default::default()
    };
    let registry = skill_reg.insert(&db).await.unwrap();

    // Now create a skill
    let skill = skills::ActiveModel {
        name: Set("test-skill".to_string()),
        skill_registry_id: Set(registry.id),
        latest_version: Set(None),
        is_active: Set(1),
        created_at: Set(chrono::Utc::now().naive_utc()),
        updated_at: Set(chrono::Utc::now().naive_utc()),
        ..Default::default()
    };

    let result = skill.insert(&db).await;
    assert!(
        result.is_ok(),
        "Failed to insert into skills: {:?}",
        result.err()
    );

    let count = skills::Entity::find().count(&db).await;
    assert!(count.is_ok());
    assert_eq!(count.unwrap(), 1);
}

#[tokio::test]
async fn test_users_entity_matches_schema() {
    let db = setup_test_db().await;

    use common::entities::users;
    use sea_orm::{ActiveModelTrait, Set};
    use uuid::Uuid;

    let user = users::ActiveModel {
        user_id: Set(Uuid::new_v4()),
        status: Set(common::entities::users::UserStatus::Active),
        role: Set(common::entities::users::UserRole::User),
        username: Set(None),
        display_name: Set(None),
        primary_email: Set(None),
        created_at: Set(chrono::Utc::now().naive_utc()),
        updated_at: Set(chrono::Utc::now().naive_utc()),
    };

    let result = user.insert(&db).await;
    assert!(
        result.is_ok(),
        "Failed to insert into users: {:?}",
        result.err()
    );

    let count = users::Entity::find().count(&db).await;
    assert!(count.is_ok());
    assert_eq!(count.unwrap(), 1);
}

#[tokio::test]
async fn test_blacklist_entity_matches_schema() {
    let db = setup_test_db().await;

    use common::entities::blacklist;
    use sea_orm::{ActiveModelTrait, Set};

    let entry = blacklist::ActiveModel {
        repository_url: Set("https://github.com/bad/repo".to_string()),
        reason: Set("spam".to_string()),
        created_at: Set(chrono::Utc::now().naive_utc()),
        ..Default::default()
    };

    let result = entry.insert(&db).await;
    assert!(
        result.is_ok(),
        "Failed to insert into blacklist: {:?}",
        result.err()
    );

    let count = blacklist::Entity::find().count(&db).await;
    assert!(count.is_ok());
    assert_eq!(count.unwrap(), 1);
}

#[tokio::test]
async fn test_organizations_entity_matches_schema() {
    let db = setup_test_db().await;

    use common::entities::organizations;
    use sea_orm::{ActiveModelTrait, Set};
    use uuid::Uuid;

    let org = organizations::ActiveModel {
        org_id: Set(Uuid::new_v4()),
        name: Set("Test Org".to_string()),
        slug: Set("test-org".to_string()),
        created_at: Set(chrono::Utc::now().naive_utc()),
        updated_at: Set(chrono::Utc::now().naive_utc()),
    };

    let result = org.insert(&db).await;
    assert!(
        result.is_ok(),
        "Failed to insert into organizations: {:?}",
        result.err()
    );

    let count = organizations::Entity::find().count(&db).await;
    assert!(count.is_ok());
    assert_eq!(count.unwrap(), 1);
}

#[tokio::test]
async fn test_refresh_tokens_entity_matches_schema() {
    let db = setup_test_db().await;

    use common::entities::refresh_tokens;
    use common::entities::users;
    use sea_orm::{ActiveModelTrait, Set};
    use uuid::Uuid;

    // First create a user
    let user = users::ActiveModel {
        user_id: Set(Uuid::new_v4()),
        status: Set(common::entities::users::UserStatus::Active),
        role: Set(common::entities::users::UserRole::User),
        username: Set(None),
        display_name: Set(None),
        primary_email: Set(None),
        created_at: Set(chrono::Utc::now().naive_utc()),
        updated_at: Set(chrono::Utc::now().naive_utc()),
    };
    let user_record = user.insert(&db).await.unwrap();

    // Create a refresh token
    let token = refresh_tokens::ActiveModel {
        user_id: Set(user_record.user_id),
        token_hash: Set("abc123".to_string()),
        rotated_from: Set(None),
        expires_at: Set(chrono::Utc::now().naive_utc()),
        revoked_at: Set(None),
        user_agent: Set(None),
        ip: Set(None),
        created_at: Set(chrono::Utc::now().naive_utc()),
        ..Default::default()
    };

    let result = token.insert(&db).await;
    assert!(
        result.is_ok(),
        "Failed to insert into refresh_tokens: {:?}",
        result.err()
    );

    let count = refresh_tokens::Entity::find().count(&db).await;
    assert!(count.is_ok());
    assert_eq!(count.unwrap(), 1);
}

#[tokio::test]
async fn test_plugins_entity_matches_schema() {
    let db = setup_test_db().await;

    use common::entities::plugins;
    use common::entities::skill_registry;
    use sea_orm::{ActiveModelTrait, Set};

    // First create a skill_registry entry
    let skill_reg = skill_registry::ActiveModel {
        platform: Set(common::entities::skill_registry::Platform::Github),
        owner: Set("test-owner".to_string()),
        name: Set("test-repo".to_string()),
        url: Set("https://github.com/test-owner/test-repo".to_string()),
        description: Set(None),
        repo_type: Set(None),
        status: Set("active".to_string()),
        blacklist_reason: Set(None),
        blacklisted_at: Set(None),
        stars: Set(0),
        last_scanned_at: Set(None),
        created_at: Set(chrono::Utc::now().naive_utc()),
        updated_at: Set(chrono::Utc::now().naive_utc()),
        ..Default::default()
    };
    let registry = skill_reg.insert(&db).await.unwrap();

    // Create a plugin
    let plugin = plugins::ActiveModel {
        skill_registry_id: Set(registry.id),
        name: Set("test-plugin".to_string()),
        description: Set(None),
        source: Set(None),
        strict: Set(0),
        latest_version: Set(None),
        is_active: Set(1),
        created_at: Set(chrono::Utc::now().naive_utc()),
        updated_at: Set(chrono::Utc::now().naive_utc()),
        ..Default::default()
    };

    let result = plugin.insert(&db).await;
    assert!(
        result.is_ok(),
        "Failed to insert into plugins: {:?}",
        result.err()
    );

    let count = plugins::Entity::find().count(&db).await;
    assert!(count.is_ok());
    assert_eq!(count.unwrap(), 1);
}

#[tokio::test]
async fn test_task_logs_entity_matches_schema() {
    let db = setup_test_db().await;

    use common::entities::task_logs;
    use sea_orm::{ActiveModelTrait, Set};

    let log = task_logs::ActiveModel {
        task_name: Set("test-task".to_string()),
        status: Set("running".to_string()),
        details: Set(None),
        started_at: Set(chrono::Utc::now().naive_utc()),
        ended_at: Set(None),
        ..Default::default()
    };

    let result = log.insert(&db).await;
    assert!(
        result.is_ok(),
        "Failed to insert into task_logs: {:?}",
        result.err()
    );

    let count = task_logs::Entity::find().count(&db).await;
    assert!(count.is_ok());
    assert_eq!(count.unwrap(), 1);
}
