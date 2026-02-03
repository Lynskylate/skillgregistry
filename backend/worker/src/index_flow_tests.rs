use crate::activities::discovery::DiscoveryService;
use crate::activities::sync::SyncService;
use crate::github::{GithubOwner, GithubRepo};
use crate::ports::{MockGithubApi, MockStorage};
use anyhow::Result;
use common::entities::{prelude::*, skill_registry, *};
use migration::MigratorTrait;
use sea_orm::{ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter};
use std::io::Write;
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

async fn setup_db() -> Result<DatabaseConnection> {
    let db = Database::connect("sqlite::memory:").await?;
    migration::Migrator::up(&db, None).await?;
    Ok(db)
}

#[tokio::test]
async fn index_flow_discovers_and_syncs_standalone_repo() -> Result<()> {
    let db = setup_db().await?;

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

    let zip_data = create_zip(vec![(
        "skill-a/SKILL.md",
        b"---\nname: test-skill\ndescription: test description\nmetadata:\n  version: 1.0.0\n---\n# Body\n",
    )]);
    github
        .expect_download_zipball()
        .returning(move |owner, repo| {
            assert_eq!(owner, "test-owner");
            assert_eq!(repo, "standalone");
            Ok(zip_data.clone())
        });

    let mut s3 = MockStorage::new();
    s3.expect_upload()
        .times(1)
        .returning(|_, _| Ok("https://oss.example/test.zip".to_string()));

    let discovery =
        DiscoveryService::run(&db, &github, vec!["topic:agent-skill".to_string()]).await?;
    assert_eq!(discovery.new_count, 1);

    let registry = SkillRegistry::find()
        .filter(skill_registry::Column::Owner.eq("test-owner"))
        .filter(skill_registry::Column::Name.eq("standalone"))
        .one(&db)
        .await?
        .unwrap();

    let pending = SyncService::fetch_pending(&db).await?;
    assert_eq!(pending, vec![registry.id]);

    let res = SyncService::process_one(&db, &s3, &github, registry.id).await?;
    assert!(matches!(res.status.as_str(), "Updated" | "Unchanged"));

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
    let db = setup_db().await?;

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
    let zip_data = create_zip(vec![
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
        .expect_download_zipball()
        .returning(move |owner, repo| {
            assert_eq!(owner, "test-owner");
            assert_eq!(repo, "market");
            Ok(zip_data.clone())
        });

    let mut s3 = MockStorage::new();
    s3.expect_upload()
        .times(1)
        .returning(|_, _| Ok("https://oss.example/p1.zip".to_string()));

    let discovery =
        DiscoveryService::run(&db, &github, vec!["topic:agent-skill".to_string()]).await?;
    assert_eq!(discovery.new_count, 1);

    let registry = SkillRegistry::find()
        .filter(skill_registry::Column::Owner.eq("test-owner"))
        .filter(skill_registry::Column::Name.eq("market"))
        .one(&db)
        .await?
        .unwrap();

    let pending = SyncService::fetch_pending(&db).await?;
    assert_eq!(pending, vec![registry.id]);

    let res = SyncService::process_one(&db, &s3, &github, registry.id).await?;
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
