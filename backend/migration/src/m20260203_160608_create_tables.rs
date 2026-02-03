use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SkillRegistry table
        manager
            .create_table(
                Table::create()
                    .table("skill_registry")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(string("platform"))
                    .col(string("owner"))
                    .col(string("name"))
                    .col(string("url"))
                    .col(text_null("description"))
                    .col(integer("stars"))
                    .col(timestamp_null("last_scanned_at"))
                    .col(timestamp("created_at"))
                    .col(timestamp("updated_at"))
                    .to_owned(),
            )
            .await?;

        // Skills table
        manager
            .create_table(
                Table::create()
                    .table("skills")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(string("name"))
                    .col(integer("skill_registry_id"))
                    .col(string_null("latest_version"))
                    .col(timestamp("created_at"))
                    .col(timestamp("updated_at"))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-skills-skill_registry_id")
                            .from("skills", "skill_registry_id")
                            .to("skill_registry", "id"),
                    )
                    .to_owned(),
            )
            .await?;

        // SkillVersions table
        manager
            .create_table(
                Table::create()
                    .table("skill_versions")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(integer("skill_id"))
                    .col(string("version"))
                    .col(text_null("description"))
                    .col(text_null("readme_content"))
                    .col(string_null("s3_key"))
                    .col(string_null("oss_url"))
                    .col(string_null("file_hash"))
                    .col(json_null("metadata"))
                    .col(timestamp("created_at"))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-skill_versions-skill_id")
                            .from("skill_versions", "skill_id")
                            .to("skills", "id"),
                    )
                    .to_owned(),
            )
            .await?;

        // TaskLogs table
        manager
            .create_table(
                Table::create()
                    .table("task_logs")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(string("task_name"))
                    .col(string("status"))
                    .col(text_null("details"))
                    .col(timestamp("started_at"))
                    .col(timestamp_null("ended_at"))
                    .to_owned(),
            )
            .await?;

        // Blacklist table
        manager
            .create_table(
                Table::create()
                    .table("blacklist")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(string("repository_url"))
                    .col(string("reason"))
                    .col(timestamp("created_at"))
                    .index(
                        Index::create()
                            .name("idx-blacklist-repository-url")
                            .col("repository_url")
                            .unique(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("blacklist").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table("task_logs").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table("skill_versions").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table("skills").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table("skill_registry").to_owned())
            .await?;

        Ok(())
    }
}
