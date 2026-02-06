use crate::activities::discovery::DiscoveryActivities;
use crate::github::{GithubOwner, GithubRepo};
use crate::ports::{MockGithubApi, MockStorage};
use crate::sync::SyncService;
use anyhow::Result;
use common::build_all;
use common::entities::{prelude::*, skill_registry, *};
use common::settings::Settings;
use migration::MigratorTrait;
use sea_orm::{ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::BTreeMap;
use std::io::Write;
use std::sync::Arc;
use zip::write::FileOptions;

fn create_zip(files: Vec<(&str, &[u8])>) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (path, content) in files {
            zip.start_file(path, options).unwrap();
            zip.write_all(content).unwrap();
        }
        zip.finish().unwrap();
    }
    buf
}

fn create_file_map(files: Vec<(&str, &[u8])>) -> BTreeMap<String, Vec<u8>> {
    files
        .into_iter()
        .map(|(path, content)| (path.to_string(), content.to_vec()))
        .collect()
}

async fn setup_db() -> Result<(DatabaseConnection, common::Services)> {
    let db = Database::connect("sqlite::memory:").await?;
    migration::Migrator::up(&db, None).await?;

    let settings = Settings::new().unwrap_or_else(|_| Settings {
        port: 3000,
        database: common::settings::DatabaseSettings {
            url: "sqlite::memory:".to_string(),
        },
        s3: common::settings::S3Settings {
            bucket: "test".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            force_path_style: false,
        },
        github: common::settings::GithubSettings {
            search_keywords: "topic:agent-skill".to_string(),
            token: None,
            api_url: "https://api.github.com".to_string(),
        },
        worker: common::settings::WorkerSettings {
            scan_interval_seconds: 3600,
        },
        temporal: common::settings::TemporalSettings {
            server_url: "http://localhost:7233".to_string(),
            task_queue: "test".to_string(),
        },
        auth: common::settings::AuthSettings::default(),
        debug: true,
    });

    let db_arc = std::sync::Arc::new(db.clone());
    let (_repos, services) = build_all(db_arc, &settings).await;

    Ok((db, services))
}

#[tokio::test]
async fn index_flow_discovers_and_syncs_standalone_repo() -> Result<()> {
    let (db, services) = setup_db().await?;

    let mut github = MockGithubApi::new();
    github.expect_search_repositories().returning(|q| {
        assert_eq!(q, "topic:agent-skill fork:false sort:updated");
        Ok(vec![GithubRepo {
            name: "standalone".to_string(),
            html_url: "https://github.com/test-owner/standalone".to_string(),
            description: Some("desc".to_string()),
            stargazers_count: 10,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            owner: GithubOwner {
                login: "test-owner".to_string(),
            },
        }])
    });

    let file_map = create_file_map(vec![(
        "skill-a/SKILL.md",
        b"---\nname: test-skill\ndescription: test description\nmetadata:\n  version: 1.0.0\n---\n# Body\n",
    )]);
    github
        .expect_clone_repository_files()
        .returning(move |owner, repo, token| {
            assert_eq!(owner, "test-owner");
            assert_eq!(repo, "standalone");
            assert!(token.is_none());
            Ok(file_map.clone())
        });

    let mut s3 = MockStorage::new();
    s3.expect_upload()
        .times(1)
        .returning(|_, _| Ok("https://oss.example/test.zip".to_string()));

    let github_arc = Arc::new(github);
    let discovery = DiscoveryActivities::new(Arc::new(db.clone()), github_arc.clone());
    let discovery_result = discovery
        .discover_repos(vec!["topic:agent-skill".to_string()])
        .await
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
    assert_eq!(discovery_result.new_count, 1);

    let registry = SkillRegistry::find()
        .filter(skill_registry::Column::Owner.eq("test-owner"))
        .filter(skill_registry::Column::Name.eq("standalone"))
        .one(&db)
        .await?
        .unwrap();
    tracing::debug!(
        registry_status = %registry.status,
        registry_repo_type = ?registry.repo_type,
        "After discovery (standalone)"
    );

    let sync_service = SyncService::new(
        db.clone(),
        Arc::new(s3),
        github_arc.clone(),
        services.registry_service.clone(),
        services.discovery_registry_service.clone(),
    );

    let pending = sync_service.fetch_pending().await?;
    assert_eq!(pending, vec![registry.id]);

    let _res = sync_service.process_one(registry.id).await?;

    let updated = SkillRegistry::find_by_id(registry.id)
        .one(&db)
        .await?
        .unwrap();
    assert_eq!(updated.repo_type.as_deref(), Some("skill"));
    assert_eq!(updated.status, "active");

    let skills = Skills::find()
        .filter(skills::Column::SkillRegistryId.eq(registry.id))
        .all(&db)
        .await?;
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "test-skill");

    let versions = SkillVersions::find()
        .filter(skill_versions::Column::SkillId.eq(skills[0].id))
        .all(&db)
        .await?;
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version, "1.0.0");

    Ok(())
}

#[tokio::test]
async fn index_flow_discovers_and_syncs_marketplace_repo() -> Result<()> {
    let (db, services) = setup_db().await?;

    let mut github = MockGithubApi::new();
    github.expect_search_repositories().returning(|q| {
        assert_eq!(q, "topic:agent-skill fork:false sort:updated");
        Ok(vec![GithubRepo {
            name: "market".to_string(),
            html_url: "https://github.com/test-owner/market".to_string(),
            description: Some("desc".to_string()),
            stargazers_count: 10,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            owner: GithubOwner {
                login: "test-owner".to_string(),
            },
        }])
    });

    let marketplace_json = br#"{
  "name": "test-market",
  "plugins": [
    {
      "name": "p1",
      "description": "plugin one",
      "source": "./plugins/p1",
      "strict": true
    }
  ]
}"#;
    let plugin_json = br#"{
  "name": "p1",
  "version": "1.2.3",
  "description": "plugin one"
}"#;
    let file_map = create_file_map(vec![
        (".claude-plugin/marketplace.json", marketplace_json),
        ("plugins/p1/.claude-plugin/plugin.json", plugin_json),
        (
            "plugins/p1/commands/hello.md",
            b"---\nname: hello\ndescription: hi\n---\n# cmd\n",
        ),
        (
            "plugins/p1/agents/reviewer.md",
            b"---\nname: reviewer\ndescription: reviews\n---\n# agent\n",
        ),
        (
            "plugins/p1/skills/s1/SKILL.md",
            b"---\nname: s1\ndescription: s1 desc\n---\n# skill\n",
        ),
    ]);
    github
        .expect_clone_repository_files()
        .returning(move |owner, repo, token| {
            assert_eq!(owner, "test-owner");
            assert_eq!(repo, "market");
            assert!(token.is_none());
            Ok(file_map.clone())
        });

    let mut s3 = MockStorage::new();
    s3.expect_upload()
        .times(1)
        .returning(|_, _| Ok("https://oss.example/p1.zip".to_string()));

    let github_arc = Arc::new(github);
    let discovery = DiscoveryActivities::new(Arc::new(db.clone()), github_arc.clone());
    let discovery_result = discovery
        .discover_repos(vec!["topic:agent-skill".to_string()])
        .await
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
    assert_eq!(discovery_result.new_count, 1);

    let registry = SkillRegistry::find()
        .filter(skill_registry::Column::Owner.eq("test-owner"))
        .filter(skill_registry::Column::Name.eq("market"))
        .one(&db)
        .await?
        .unwrap();
    tracing::debug!(
        registry_status = %registry.status,
        registry_repo_type = ?registry.repo_type,
        "After discovery (marketplace)"
    );

    let sync_service = SyncService::new(
        db.clone(),
        Arc::new(s3),
        github_arc.clone(),
        services.registry_service.clone(),
        services.discovery_registry_service.clone(),
    );

    let pending = sync_service.fetch_pending().await?;
    assert_eq!(pending, vec![registry.id]);

    let res = sync_service.process_one(registry.id).await?;
    tracing::debug!(status = %res.status, version = ?res.version, "After sync (marketplace)");
    assert!(matches!(res.status.as_str(), "Updated" | "Unchanged"));

    let updated = SkillRegistry::find_by_id(registry.id)
        .one(&db)
        .await?
        .unwrap();
    assert_eq!(updated.repo_type.as_deref(), Some("marketplace"));
    assert_eq!(updated.status, "active");

    let plugins = Plugins::find()
        .filter(plugins::Column::SkillRegistryId.eq(registry.id))
        .all(&db)
        .await?;
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].name, "p1");

    Ok(())
}
