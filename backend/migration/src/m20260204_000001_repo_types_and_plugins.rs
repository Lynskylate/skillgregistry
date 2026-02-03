use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table("skill_registry")
                    .add_column(string_null("repo_type"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table("skill_registry")
                    .add_column(
                        ColumnDef::new(Alias::new("status"))
                            .string()
                            .not_null()
                            .default("active"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table("skill_registry")
                    .add_column(text_null("blacklist_reason"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table("skill_registry")
                    .add_column(timestamp_null("blacklisted_at"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table("skills")
                    .add_column(
                        ColumnDef::new(Alias::new("is_active"))
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-skills-registry-name-unique")
                    .table("skills")
                    .col("skill_registry_id")
                    .col("name")
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-skill-versions-skill-version-unique")
                    .table("skill_versions")
                    .col("skill_id")
                    .col("version")
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table("plugins")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(integer("skill_registry_id"))
                    .col(string("name"))
                    .col(text_null("description"))
                    .col(json_null("source"))
                    .col(
                        ColumnDef::new(Alias::new("strict"))
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(string_null("latest_version"))
                    .col(
                        ColumnDef::new(Alias::new("is_active"))
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(timestamp("created_at"))
                    .col(timestamp("updated_at"))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-plugins-skill_registry_id")
                            .from("plugins", "skill_registry_id")
                            .to("skill_registry", "id"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-plugins-registry-name-unique")
                    .table("plugins")
                    .col("skill_registry_id")
                    .col("name")
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table("plugin_versions")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(integer("plugin_id"))
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
                            .name("fk-plugin_versions-plugin_id")
                            .from("plugin_versions", "plugin_id")
                            .to("plugins", "id"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-plugin-versions-plugin-version-unique")
                    .table("plugin_versions")
                    .col("plugin_id")
                    .col("version")
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table("plugin_components")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(integer("plugin_version_id"))
                    .col(string("kind"))
                    .col(string("path"))
                    .col(string("name"))
                    .col(text_null("description"))
                    .col(text_null("markdown_content"))
                    .col(json_null("metadata"))
                    .col(timestamp("created_at"))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-plugin_components-plugin_version_id")
                            .from("plugin_components", "plugin_version_id")
                            .to("plugin_versions", "id"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-plugin-components-version-kind-path-unique")
                    .table("plugin_components")
                    .col("plugin_version_id")
                    .col("kind")
                    .col("path")
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-plugin-components-version-kind-name")
                    .table("plugin_components")
                    .col("plugin_version_id")
                    .col("kind")
                    .col("name")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx-plugin-components-version-kind-name")
                    .table("plugin_components")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-plugin-components-version-kind-path-unique")
                    .table("plugin_components")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table("plugin_components").to_owned())
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-plugin-versions-plugin-version-unique")
                    .table("plugin_versions")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table("plugin_versions").to_owned())
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-plugins-registry-name-unique")
                    .table("plugins")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table("plugins").to_owned())
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-skill-versions-skill-version-unique")
                    .table("skill_versions")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-skills-registry-name-unique")
                    .table("skills")
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table("skills")
                    .drop_column(Alias::new("is_active"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table("skill_registry")
                    .drop_column(Alias::new("repo_type"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table("skill_registry")
                    .drop_column(Alias::new("status"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table("skill_registry")
                    .drop_column(Alias::new("blacklist_reason"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table("skill_registry")
                    .drop_column(Alias::new("blacklisted_at"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
