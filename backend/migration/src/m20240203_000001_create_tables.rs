use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SkillRegistry
        manager
            .create_table(
                Table::create()
                    .table(SkillRegistry::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SkillRegistry::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SkillRegistry::Platform).string().not_null())
                    .col(ColumnDef::new(SkillRegistry::Owner).string().not_null())
                    .col(ColumnDef::new(SkillRegistry::Name).string().not_null())
                    .col(ColumnDef::new(SkillRegistry::Url).string().not_null())
                    .col(ColumnDef::new(SkillRegistry::Description).text())
                    .col(ColumnDef::new(SkillRegistry::Stars).integer().not_null())
                    .col(ColumnDef::new(SkillRegistry::LastScannedAt).timestamp())
                    .col(
                        ColumnDef::new(SkillRegistry::CreatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SkillRegistry::UpdatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Skills
        manager
            .create_table(
                Table::create()
                    .table(Skills::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Skills::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Skills::Name).string().not_null())
                    .col(ColumnDef::new(Skills::SkillRegistryId).integer().not_null())
                    .col(ColumnDef::new(Skills::LatestVersion).string())
                    .col(ColumnDef::new(Skills::CreatedAt).timestamp().not_null())
                    .col(ColumnDef::new(Skills::UpdatedAt).timestamp().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-skills-skill_registry_id")
                            .from(Skills::Table, Skills::SkillRegistryId)
                            .to(SkillRegistry::Table, SkillRegistry::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // SkillVersions
        manager
            .create_table(
                Table::create()
                    .table(SkillVersions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SkillVersions::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SkillVersions::SkillId).integer().not_null())
                    .col(ColumnDef::new(SkillVersions::Version).string().not_null())
                    .col(ColumnDef::new(SkillVersions::Description).text())
                    .col(ColumnDef::new(SkillVersions::ReadmeContent).text())
                    .col(ColumnDef::new(SkillVersions::S3Key).string())
                    .col(ColumnDef::new(SkillVersions::OssUrl).string())
                    .col(ColumnDef::new(SkillVersions::FileHash).string())
                    .col(ColumnDef::new(SkillVersions::Metadata).json())
                    .col(
                        ColumnDef::new(SkillVersions::CreatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-skill_versions-skill_id")
                            .from(SkillVersions::Table, SkillVersions::SkillId)
                            .to(Skills::Table, Skills::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // TaskLogs
        manager
            .create_table(
                Table::create()
                    .table(TaskLogs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TaskLogs::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TaskLogs::TaskName).string().not_null())
                    .col(ColumnDef::new(TaskLogs::Status).string().not_null())
                    .col(ColumnDef::new(TaskLogs::Details).text())
                    .col(ColumnDef::new(TaskLogs::StartedAt).timestamp().not_null())
                    .col(ColumnDef::new(TaskLogs::EndedAt).timestamp())
                    .to_owned(),
            )
            .await?;

        // Blacklist
        manager
            .create_table(
                Table::create()
                    .table(Blacklist::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Blacklist::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Blacklist::RepositoryUrl)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Blacklist::Reason).string().not_null())
                    .col(ColumnDef::new(Blacklist::CreatedAt).timestamp().not_null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Blacklist::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(TaskLogs::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SkillVersions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Skills::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SkillRegistry::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum SkillRegistry {
    Table,
    Id,
    Platform,
    Owner,
    Name,
    Url,
    Description,
    Stars,
    LastScannedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Skills {
    Table,
    Id,
    Name,
    SkillRegistryId,
    LatestVersion,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum SkillVersions {
    Table,
    Id,
    SkillId,
    Version,
    Description,
    ReadmeContent,
    S3Key,
    OssUrl,
    FileHash,
    Metadata,
    CreatedAt,
}

#[derive(DeriveIden)]
enum TaskLogs {
    Table,
    Id,
    TaskName,
    Status,
    Details,
    StartedAt,
    EndedAt,
}

#[derive(DeriveIden)]
enum Blacklist {
    Table,
    Id,
    RepositoryUrl,
    Reason,
    CreatedAt,
}
