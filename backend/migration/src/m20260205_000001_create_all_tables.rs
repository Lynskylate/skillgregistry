use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SkillRegistry::Table)
                    .col(pk_auto(SkillRegistry::Id))
                    .col(string_len(SkillRegistry::Platform, 256))
                    .col(string(SkillRegistry::Owner))
                    .col(string(SkillRegistry::Name))
                    .col(string(SkillRegistry::Url))
                    .col(text_null(SkillRegistry::Description))
                    .col(string_null(SkillRegistry::RepoType))
                    .col(string(SkillRegistry::Status))
                    .col(text_null(SkillRegistry::BlacklistReason))
                    .col(date_time_null(SkillRegistry::BlacklistedAt))
                    .col(integer(SkillRegistry::Stars))
                    .col(date_time_null(SkillRegistry::LastScannedAt))
                    .col(date_time(SkillRegistry::CreatedAt))
                    .col(date_time(SkillRegistry::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Skills::Table)
                    .col(pk_auto(Skills::Id))
                    .col(string(Skills::Name))
                    .col(integer(Skills::SkillRegistryId))
                    .col(string_null(Skills::LatestVersion))
                    .col(integer(Skills::IsActive))
                    .col(date_time(Skills::CreatedAt))
                    .col(date_time(Skills::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_skills_skill_registry_id")
                            .from(Skills::Table, Skills::SkillRegistryId)
                            .to(SkillRegistry::Table, SkillRegistry::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SkillVersions::Table)
                    .col(pk_auto(SkillVersions::Id))
                    .col(integer(SkillVersions::SkillId))
                    .col(string(SkillVersions::Version))
                    .col(text_null(SkillVersions::Description))
                    .col(text_null(SkillVersions::ReadmeContent))
                    .col(string_null(SkillVersions::S3Key))
                    .col(string_null(SkillVersions::OssUrl))
                    .col(string_null(SkillVersions::FileHash))
                    .col(json_null(SkillVersions::Metadata))
                    .col(date_time(SkillVersions::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_skill_versions_skill_id")
                            .from(SkillVersions::Table, SkillVersions::SkillId)
                            .to(Skills::Table, Skills::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Blacklist::Table)
                    .col(pk_auto(Blacklist::Id))
                    .col(string_uniq(Blacklist::RepositoryUrl))
                    .col(string(Blacklist::Reason))
                    .col(date_time(Blacklist::CreatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .col(uuid(Users::UserId).primary_key())
                    .col(string_len(Users::Status, 32))
                    .col(string_len(Users::Role, 32))
                    .col(string_null(Users::Username).unique_key())
                    .col(string_null(Users::DisplayName))
                    .col(string_null(Users::PrimaryEmail).unique_key())
                    .col(date_time(Users::CreatedAt))
                    .col(date_time(Users::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(LocalCredentials::Table)
                    .col(uuid(LocalCredentials::UserId).primary_key())
                    .col(string(LocalCredentials::PasswordHash))
                    .col(date_time(LocalCredentials::PasswordUpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_local_credentials_user_id")
                            .from(LocalCredentials::Table, LocalCredentials::UserId)
                            .to(Users::Table, Users::UserId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(RefreshTokens::Table)
                    .col(pk_auto(RefreshTokens::Id))
                    .col(uuid(RefreshTokens::UserId))
                    .col(string(RefreshTokens::TokenHash))
                    .col(big_integer_null(RefreshTokens::RotatedFrom))
                    .col(date_time(RefreshTokens::ExpiresAt))
                    .col(date_time_null(RefreshTokens::RevokedAt))
                    .col(string_null(RefreshTokens::UserAgent))
                    .col(string_null(RefreshTokens::Ip))
                    .col(date_time(RefreshTokens::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_refresh_tokens_user_id")
                            .from(RefreshTokens::Table, RefreshTokens::UserId)
                            .to(Users::Table, Users::UserId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Organizations::Table)
                    .col(uuid(Organizations::OrgId).primary_key())
                    .col(string(Organizations::Name))
                    .col(string_uniq(Organizations::Slug))
                    .col(date_time(Organizations::CreatedAt))
                    .col(date_time(Organizations::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(OrgMemberships::Table)
                    .col(pk_auto(OrgMemberships::Id))
                    .col(uuid(OrgMemberships::OrgId))
                    .col(uuid(OrgMemberships::UserId))
                    .col(string_len(OrgMemberships::OrgRole, 32))
                    .col(date_time(OrgMemberships::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_org_memberships_org_id")
                            .from(OrgMemberships::Table, OrgMemberships::OrgId)
                            .to(Organizations::Table, Organizations::OrgId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_org_memberships_user_id")
                            .from(OrgMemberships::Table, OrgMemberships::UserId)
                            .to(Users::Table, Users::UserId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SsoConnections::Table)
                    .col(uuid(SsoConnections::ConnectionId).primary_key())
                    .col(uuid(SsoConnections::OrgId))
                    .col(string_len(SsoConnections::Protocol, 32))
                    .col(string_null(SsoConnections::Issuer))
                    .col(string_null(SsoConnections::MetadataUrl))
                    .col(string_null(SsoConnections::SsoUrl))
                    .col(string_null(SsoConnections::X509CertFingerprint))
                    .col(string_null(SsoConnections::ClientId))
                    .col(string_null(SsoConnections::ClientSecret))
                    .col(text_null(SsoConnections::AllowedDomainsJson))
                    .col(boolean(SsoConnections::Enabled))
                    .col(date_time(SsoConnections::CreatedAt))
                    .col(date_time(SsoConnections::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sso_connections_org_id")
                            .from(SsoConnections::Table, SsoConnections::OrgId)
                            .to(Organizations::Table, Organizations::OrgId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SsoIdentities::Table)
                    .col(pk_auto(SsoIdentities::Id))
                    .col(uuid(SsoIdentities::ConnectionId))
                    .col(string(SsoIdentities::ProviderUserId))
                    .col(uuid(SsoIdentities::UserId))
                    .col(string_null(SsoIdentities::Email))
                    .col(boolean(SsoIdentities::EmailVerified))
                    .col(string_null(SsoIdentities::DisplayName))
                    .col(date_time(SsoIdentities::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sso_identities_connection_id")
                            .from(SsoIdentities::Table, SsoIdentities::ConnectionId)
                            .to(SsoConnections::Table, SsoConnections::ConnectionId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sso_identities_user_id")
                            .from(SsoIdentities::Table, SsoIdentities::UserId)
                            .to(Users::Table, Users::UserId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(AuthIdentities::Table)
                    .col(pk_auto(AuthIdentities::Id))
                    .col(uuid(AuthIdentities::UserId))
                    .col(string_len(AuthIdentities::Provider, 64))
                    .col(string(AuthIdentities::ProviderUserId))
                    .col(string_null(AuthIdentities::Email))
                    .col(boolean(AuthIdentities::EmailVerified))
                    .col(string_null(AuthIdentities::DisplayName))
                    .col(date_time(AuthIdentities::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_auth_identities_user_id")
                            .from(AuthIdentities::Table, AuthIdentities::UserId)
                            .to(Users::Table, Users::UserId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Plugins::Table)
                    .col(pk_auto(Plugins::Id))
                    .col(integer(Plugins::SkillRegistryId))
                    .col(string(Plugins::Name))
                    .col(text_null(Plugins::Description))
                    .col(json_null(Plugins::Source))
                    .col(integer(Plugins::Strict))
                    .col(string_null(Plugins::LatestVersion))
                    .col(integer(Plugins::IsActive))
                    .col(date_time(Plugins::CreatedAt))
                    .col(date_time(Plugins::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_plugins_skill_registry_id")
                            .from(Plugins::Table, Plugins::SkillRegistryId)
                            .to(SkillRegistry::Table, SkillRegistry::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PluginVersions::Table)
                    .col(pk_auto(PluginVersions::Id))
                    .col(integer(PluginVersions::PluginId))
                    .col(string(PluginVersions::Version))
                    .col(text_null(PluginVersions::Description))
                    .col(text_null(PluginVersions::ReadmeContent))
                    .col(string_null(PluginVersions::S3Key))
                    .col(string_null(PluginVersions::OssUrl))
                    .col(string_null(PluginVersions::FileHash))
                    .col(json_null(PluginVersions::Metadata))
                    .col(date_time(PluginVersions::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_plugin_versions_plugin_id")
                            .from(PluginVersions::Table, PluginVersions::PluginId)
                            .to(Plugins::Table, Plugins::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PluginComponents::Table)
                    .col(pk_auto(PluginComponents::Id))
                    .col(integer(PluginComponents::PluginVersionId))
                    .col(string(PluginComponents::Kind))
                    .col(string(PluginComponents::Path))
                    .col(string(PluginComponents::Name))
                    .col(text_null(PluginComponents::Description))
                    .col(text_null(PluginComponents::MarkdownContent))
                    .col(json_null(PluginComponents::Metadata))
                    .col(date_time(PluginComponents::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_plugin_components_plugin_version_id")
                            .from(PluginComponents::Table, PluginComponents::PluginVersionId)
                            .to(PluginVersions::Table, PluginVersions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TaskLogs::Table)
                    .col(pk_auto(TaskLogs::Id))
                    .col(string(TaskLogs::TaskName))
                    .col(string(TaskLogs::Status))
                    .col(text_null(TaskLogs::Details))
                    .col(date_time(TaskLogs::StartedAt))
                    .col(date_time_null(TaskLogs::EndedAt))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TaskLogs::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PluginComponents::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PluginVersions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Plugins::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(AuthIdentities::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SsoIdentities::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SsoConnections::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(OrgMemberships::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Organizations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RefreshTokens::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(LocalCredentials::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Blacklist::Table).to_owned())
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
    RepoType,
    Status,
    BlacklistReason,
    BlacklistedAt,
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
    IsActive,
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
enum Blacklist {
    Table,
    Id,
    RepositoryUrl,
    Reason,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    UserId,
    Status,
    Role,
    Username,
    DisplayName,
    PrimaryEmail,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum LocalCredentials {
    Table,
    UserId,
    PasswordHash,
    PasswordUpdatedAt,
}

#[derive(DeriveIden)]
enum RefreshTokens {
    Table,
    Id,
    UserId,
    TokenHash,
    RotatedFrom,
    ExpiresAt,
    RevokedAt,
    UserAgent,
    Ip,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    OrgId,
    Name,
    Slug,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum OrgMemberships {
    Table,
    Id,
    OrgId,
    UserId,
    OrgRole,
    CreatedAt,
}

#[derive(DeriveIden)]
enum SsoConnections {
    Table,
    ConnectionId,
    OrgId,
    Protocol,
    Issuer,
    MetadataUrl,
    SsoUrl,
    X509CertFingerprint,
    ClientId,
    ClientSecret,
    AllowedDomainsJson,
    Enabled,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum SsoIdentities {
    Table,
    Id,
    ConnectionId,
    ProviderUserId,
    UserId,
    Email,
    EmailVerified,
    DisplayName,
    CreatedAt,
}

#[derive(DeriveIden)]
enum AuthIdentities {
    Table,
    Id,
    UserId,
    Provider,
    ProviderUserId,
    Email,
    EmailVerified,
    DisplayName,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Plugins {
    Table,
    Id,
    SkillRegistryId,
    Name,
    Description,
    Source,
    Strict,
    LatestVersion,
    IsActive,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum PluginVersions {
    Table,
    Id,
    PluginId,
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
enum PluginComponents {
    Table,
    Id,
    PluginVersionId,
    Kind,
    Path,
    Name,
    Description,
    MarkdownContent,
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
