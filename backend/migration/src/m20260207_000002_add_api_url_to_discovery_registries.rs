use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(DiscoveryRegistries::Table)
                    .add_column(
                        ColumnDef::new(DiscoveryRegistries::ApiUrl)
                            .string()
                            .not_null()
                            .default("https://api.github.com"),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(DiscoveryRegistries::Table)
                    .drop_column(DiscoveryRegistries::ApiUrl)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum DiscoveryRegistries {
    Table,
    ApiUrl,
}
