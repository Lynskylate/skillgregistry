use anyhow::Result;
use async_trait::async_trait;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client;
use common::entities::{prelude::*, *};
use common::settings::Settings;
use sea_orm::*;

fn should_force_path_style(force_from_settings: bool, endpoint: Option<&str>) -> bool {
    if force_from_settings {
        return true;
    }

    let is_aliyun = endpoint
        .map(|e| e.contains("aliyuncs.com"))
        .unwrap_or(false);

    endpoint.is_some() && !is_aliyun
}

fn normalize_endpoint(endpoint: Option<&str>) -> Option<String> {
    endpoint.map(|ep| {
        let trimmed = ep.trim_matches('"').trim();
        if trimmed.starts_with("http") {
            trimmed.to_string()
        } else {
            format!("https://{}", trimmed)
        }
    })
}

fn has_static_credentials(access_key: Option<&str>, secret_key: Option<&str>) -> bool {
    matches!((access_key, secret_key), (Some(ak), Some(sk)) if !ak.is_empty() && !sk.is_empty())
}

fn should_abort_s3_check(skills_count: u64) -> bool {
    skills_count == 0
}

#[async_trait]
trait S3ClientOps {
    async fn head_object(&self, bucket: String, s3_key: String) -> Result<()>;
    async fn list_objects(&self, bucket: String) -> Result<Vec<String>>;
}

#[async_trait]
impl S3ClientOps for Client {
    async fn head_object(&self, bucket: String, s3_key: String) -> Result<()> {
        self.head_object().bucket(bucket).key(s3_key).send().await?;
        Ok(())
    }

    async fn list_objects(&self, bucket: String) -> Result<Vec<String>> {
        let output = self.list_objects_v2().bucket(bucket).send().await?;
        Ok(output
            .contents()
            .iter()
            .filter_map(|obj| obj.key().map(ToString::to_string))
            .collect())
    }
}

fn build_s3_client(settings: &Settings) -> Client {
    let endpoint = settings.s3.endpoint.clone();
    let region = settings.s3.region.clone();
    let access_key = settings.s3.access_key_id.clone();
    let secret_key = settings.s3.secret_access_key.clone();

    let region_provider =
        RegionProviderChain::first_try(aws_config::Region::new(region)).or_default_provider();
    let mut config_loader =
        aws_config::defaults(aws_config::BehaviorVersion::latest()).region(region_provider);

    if let Some(ep) = normalize_endpoint(endpoint.as_deref()) {
        config_loader = config_loader.endpoint_url(ep);
    }

    if has_static_credentials(access_key.as_deref(), secret_key.as_deref()) {
        let creds = aws_sdk_s3::config::Credentials::new(
            access_key.unwrap_or_default(),
            secret_key.unwrap_or_default(),
            None,
            None,
            "config",
        );
        config_loader = config_loader
            .credentials_provider(aws_sdk_s3::config::SharedCredentialsProvider::new(creds));
    }

    let config = futures::executor::block_on(config_loader.load());

    let force_path_style =
        should_force_path_style(settings.s3.force_path_style, endpoint.as_deref());

    let s3_config = aws_sdk_s3::config::Builder::from(&config)
        .force_path_style(force_path_style)
        .build();
    Client::from_conf(s3_config)
}

async fn load_latest_s3_key(db: &DatabaseConnection) -> Result<Option<String>> {
    let registry_count = SkillRegistry::find().count(db).await?;
    println!("Discovered Repositories: {}", registry_count);

    let skills_count = Skills::find().count(db).await?;
    println!("Synced Skills: {}", skills_count);

    let versions_count = SkillVersions::find().count(db).await?;
    println!("Skill Versions: {}", versions_count);

    if should_abort_s3_check(skills_count) {
        println!("!!! No skills synced. Aborting S3 check. !!!");
        return Ok(None);
    }

    let version = SkillVersions::find()
        .order_by_desc(skill_versions::Column::CreatedAt)
        .one(db)
        .await?
        .expect("Should have at least one version if count > 0");

    let s3_key = version.s3_key.expect("Version should have s3_key");
    println!("Verifying S3 Key: {}", s3_key);

    Ok(Some(s3_key))
}

async fn verify_s3_object<C: S3ClientOps + Sync>(
    client: &C,
    bucket: &str,
    s3_key: &str,
) -> Result<()> {
    match client
        .head_object(bucket.to_string(), s3_key.to_string())
        .await
    {
        Ok(_) => println!("✅ S3 Verification SUCCESS: Object exists."),
        Err(e) => {
            println!("❌ S3 Verification FAILED: {}", e);
            println!("Listing bucket contents:");
            match client.list_objects(bucket.to_string()).await {
                Ok(keys) => {
                    for key in keys {
                        println!(" - {}", key);
                    }
                }
                Err(le) => println!("Failed to list bucket: {}", le),
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let settings = Settings::new()?;

    println!("=== E2E Verification Start ===");

    let db_url = settings.database.url.clone();
    println!("Connecting to DB: {}", db_url);
    let db = Database::connect(&db_url).await?;

    let Some(s3_key) = load_latest_s3_key(&db).await? else {
        return Ok(());
    };

    let bucket = settings.s3.bucket.clone();
    let client = build_s3_client(&settings);

    verify_s3_object(&client, &bucket, &s3_key).await
}

#[cfg(test)]
mod tests {
    use super::{
        has_static_credentials, load_latest_s3_key, normalize_endpoint, should_abort_s3_check,
        should_force_path_style, verify_s3_object, S3ClientOps,
    };
    use async_trait::async_trait;
    use chrono::Utc;
    use migration::MigratorTrait;
    use mockall::{mock, predicate};
    use sea_orm::{ActiveModelTrait, Database};
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvRestore {
        old_values: Vec<(String, Option<String>)>,
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (key, value) in self.old_values.drain(..).rev() {
                match value {
                    Some(v) => std::env::set_var(&key, v),
                    None => std::env::remove_var(&key),
                }
            }
        }
    }

    fn set_env_with_restore(updates: &[(String, String)]) -> EnvRestore {
        let mut old_values = Vec::new();
        for (key, value) in updates {
            old_values.push((key.clone(), std::env::var(key).ok()));
            std::env::set_var(key, value);
        }

        EnvRestore { old_values }
    }

    async fn setup_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        migration::Migrator::up(&db, None).await.unwrap();
        db
    }

    async fn seed_skill_data(db: &sea_orm::DatabaseConnection, s3_key: &str) {
        let now = Utc::now().naive_utc();
        let registry = common::entities::skill_registry::ActiveModel {
            platform: sea_orm::Set(common::entities::skill_registry::Platform::Github),
            owner: sea_orm::Set("acme".to_string()),
            name: sea_orm::Set("skills".to_string()),
            url: sea_orm::Set("https://github.com/acme/skills".to_string()),
            host: sea_orm::Set(Some("github.com".to_string())),
            description: sea_orm::Set(Some("demo".to_string())),
            repo_type: sea_orm::Set(Some("standalone".to_string())),
            status: sea_orm::Set("active".to_string()),
            blacklist_reason: sea_orm::Set(None),
            blacklisted_at: sea_orm::Set(None),
            stars: sea_orm::Set(0),
            last_scanned_at: sea_orm::Set(None),
            created_at: sea_orm::Set(now),
            updated_at: sea_orm::Set(now),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();

        let skill = common::entities::skills::ActiveModel {
            name: sea_orm::Set("demo-skill".to_string()),
            skill_registry_id: sea_orm::Set(registry.id),
            latest_version: sea_orm::Set(Some("1.0.0".to_string())),
            install_count: sea_orm::Set(0),
            is_active: sea_orm::Set(1),
            created_at: sea_orm::Set(now),
            updated_at: sea_orm::Set(now),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();

        common::entities::skill_versions::ActiveModel {
            skill_id: sea_orm::Set(skill.id),
            version: sea_orm::Set("1.0.0".to_string()),
            description: sea_orm::Set(None),
            readme_content: sea_orm::Set(None),
            s3_key: sea_orm::Set(Some(s3_key.to_string())),
            oss_url: sea_orm::Set(None),
            file_hash: sea_orm::Set(None),
            metadata: sea_orm::Set(None),
            created_at: sea_orm::Set(now),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();
    }

    mock! {
        S3Ops {}

        #[async_trait]
        impl S3ClientOps for S3Ops {
            async fn head_object(&self, bucket: String, s3_key: String) -> anyhow::Result<()>;
            async fn list_objects(&self, bucket: String) -> anyhow::Result<Vec<String>>;
        }
    }

    #[test]
    fn should_force_path_style_uses_settings_or_endpoint_rules() {
        assert!(should_force_path_style(true, None));
        assert!(should_force_path_style(
            false,
            Some("http://localhost:9000")
        ));
        assert!(!should_force_path_style(
            false,
            Some("https://oss-cn-shanghai.aliyuncs.com")
        ));
        assert!(!should_force_path_style(false, None));
    }

    #[test]
    fn endpoint_and_credentials_helpers_cover_edge_cases() {
        assert_eq!(
            normalize_endpoint(Some("s3.example.com")),
            Some("https://s3.example.com".to_string())
        );
        assert_eq!(
            normalize_endpoint(Some("\"http://localhost:9000\"")),
            Some("http://localhost:9000".to_string())
        );
        assert_eq!(normalize_endpoint(None), None);

        assert!(has_static_credentials(Some("ak"), Some("sk")));
        assert!(!has_static_credentials(Some(""), Some("sk")));
        assert!(!has_static_credentials(None, Some("sk")));

        assert!(should_abort_s3_check(0));
        assert!(!should_abort_s3_check(1));
    }

    #[tokio::test]
    async fn load_latest_s3_key_returns_none_when_no_skills() {
        let db = setup_db().await;
        let key = load_latest_s3_key(&db).await.unwrap();
        assert_eq!(key, None);
    }

    #[tokio::test]
    async fn load_latest_s3_key_returns_newest_version_key() {
        let db = setup_db().await;
        seed_skill_data(&db, "archives/demo.zip").await;

        let key = load_latest_s3_key(&db).await.unwrap();
        assert_eq!(key, Some("archives/demo.zip".to_string()));
    }

    #[tokio::test]
    async fn verify_s3_object_succeeds_when_head_returns_ok() {
        let mut mock = MockS3Ops::new();
        mock.expect_head_object()
            .times(1)
            .with(
                predicate::eq("test-bucket".to_string()),
                predicate::eq("archives/demo.zip".to_string()),
            )
            .returning(|_, _| Ok(()));

        mock.expect_list_objects().times(0);

        verify_s3_object(&mock, "test-bucket", "archives/demo.zip")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn verify_s3_object_lists_objects_when_head_fails() {
        let mut mock = MockS3Ops::new();
        mock.expect_head_object()
            .times(1)
            .returning(|_, _| Err(anyhow::anyhow!("head failed")));

        mock.expect_list_objects()
            .times(1)
            .with(predicate::eq("test-bucket".to_string()))
            .returning(|_| Ok(vec!["archives/demo.zip".to_string()]));

        verify_s3_object(&mock, "test-bucket", "archives/demo.zip")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn verify_s3_object_handles_list_failure_after_head_failure() {
        let mut mock = MockS3Ops::new();
        mock.expect_head_object()
            .times(1)
            .returning(|_, _| Err(anyhow::anyhow!("head failed")));

        mock.expect_list_objects()
            .times(1)
            .returning(|_| Err(anyhow::anyhow!("list failed")));

        verify_s3_object(&mock, "test-bucket", "archives/demo.zip")
            .await
            .unwrap();
    }

    #[test]
    fn main_exits_early_when_no_skills_exist() {
        let _guard = env_lock().lock().unwrap();

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let db_url = format!("sqlite://{}?mode=rwc", tmp.path().display());

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let db = sea_orm::Database::connect(&db_url).await.unwrap();
            migration::Migrator::up(&db, None).await.unwrap();
        });

        let _env = set_env_with_restore(&[("SKILLREGISTRY_DATABASE__URL".to_string(), db_url)]);

        let result = super::main();
        assert!(result.is_ok());
    }
}
