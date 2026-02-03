use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("users"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("user_id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("status"))
                            .string()
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(Alias::new("role"))
                            .string()
                            .not_null()
                            .default("user"),
                    )
                    .col(ColumnDef::new(Alias::new("username")).string())
                    .col(ColumnDef::new(Alias::new("display_name")).string())
                    .col(ColumnDef::new(Alias::new("primary_email")).string())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-users-username-unique")
                    .table(Alias::new("users"))
                    .col(Alias::new("username"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-users-primary-email-unique")
                    .table(Alias::new("users"))
                    .col(Alias::new("primary_email"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("auth_identities"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("user_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("provider")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("provider_user_id"))
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("email")).string())
                    .col(
                        ColumnDef::new(Alias::new("email_verified"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(Alias::new("display_name")).string())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-auth_identities-user_id")
                            .from(Alias::new("auth_identities"), Alias::new("user_id"))
                            .to(Alias::new("users"), Alias::new("user_id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-auth-identities-provider-provider_user_id-unique")
                    .table(Alias::new("auth_identities"))
                    .col(Alias::new("provider"))
                    .col(Alias::new("provider_user_id"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("local_credentials"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("user_id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("password_hash"))
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("password_updated_at"))
                            .timestamp()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-local_credentials-user_id")
                            .from(Alias::new("local_credentials"), Alias::new("user_id"))
                            .to(Alias::new("users"), Alias::new("user_id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("refresh_tokens"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("user_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("token_hash")).text().not_null())
                    .col(ColumnDef::new(Alias::new("rotated_from")).big_integer())
                    .col(
                        ColumnDef::new(Alias::new("expires_at"))
                            .timestamp()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("revoked_at")).timestamp())
                    .col(ColumnDef::new(Alias::new("user_agent")).text())
                    .col(ColumnDef::new(Alias::new("ip")).string())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-refresh_tokens-user_id")
                            .from(Alias::new("refresh_tokens"), Alias::new("user_id"))
                            .to(Alias::new("users"), Alias::new("user_id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("organizations"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("org_id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("name")).string().not_null())
                    .col(ColumnDef::new(Alias::new("slug")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-organizations-slug-unique")
                    .table(Alias::new("organizations"))
                    .col(Alias::new("slug"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("org_memberships"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("org_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("user_id")).uuid().not_null())
                    .col(
                        ColumnDef::new(Alias::new("org_role"))
                            .string()
                            .not_null()
                            .default("member"),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-org_memberships-org_id")
                            .from(Alias::new("org_memberships"), Alias::new("org_id"))
                            .to(Alias::new("organizations"), Alias::new("org_id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-org_memberships-user_id")
                            .from(Alias::new("org_memberships"), Alias::new("user_id"))
                            .to(Alias::new("users"), Alias::new("user_id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-org_memberships-org-user-unique")
                    .table(Alias::new("org_memberships"))
                    .col(Alias::new("org_id"))
                    .col(Alias::new("user_id"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("sso_connections"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("connection_id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("org_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("protocol")).string().not_null())
                    .col(ColumnDef::new(Alias::new("issuer")).string())
                    .col(ColumnDef::new(Alias::new("metadata_url")).string())
                    .col(ColumnDef::new(Alias::new("sso_url")).string())
                    .col(ColumnDef::new(Alias::new("x509_cert_fingerprint")).string())
                    .col(ColumnDef::new(Alias::new("client_id")).string())
                    .col(ColumnDef::new(Alias::new("client_secret")).string())
                    .col(ColumnDef::new(Alias::new("allowed_domains_json")).text())
                    .col(
                        ColumnDef::new(Alias::new("enabled"))
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-sso_connections-org_id")
                            .from(Alias::new("sso_connections"), Alias::new("org_id"))
                            .to(Alias::new("organizations"), Alias::new("org_id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("sso_identities"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("connection_id"))
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("provider_user_id"))
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("user_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("email")).string())
                    .col(
                        ColumnDef::new(Alias::new("email_verified"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(Alias::new("display_name")).string())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-sso_identities-connection_id")
                            .from(Alias::new("sso_identities"), Alias::new("connection_id"))
                            .to(Alias::new("sso_connections"), Alias::new("connection_id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-sso_identities-user_id")
                            .from(Alias::new("sso_identities"), Alias::new("user_id"))
                            .to(Alias::new("users"), Alias::new("user_id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-sso_identities-connection-sub-unique")
                    .table(Alias::new("sso_identities"))
                    .col(Alias::new("connection_id"))
                    .col(Alias::new("provider_user_id"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx-sso_identities-connection-sub-unique")
                    .table(Alias::new("sso_identities"))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(Alias::new("sso_identities")).to_owned())
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("sso_connections"))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-org_memberships-org-user-unique")
                    .table(Alias::new("org_memberships"))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("org_memberships"))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-organizations-slug-unique")
                    .table(Alias::new("organizations"))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(Alias::new("organizations")).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Alias::new("refresh_tokens")).to_owned())
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("local_credentials"))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-auth-identities-provider-provider_user_id-unique")
                    .table(Alias::new("auth_identities"))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("auth_identities"))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-users-primary-email-unique")
                    .table(Alias::new("users"))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-users-username-unique")
                    .table(Alias::new("users"))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(Alias::new("users")).to_owned())
            .await?;

        Ok(())
    }
}
