pub use sea_orm_migration::prelude::*;

mod m20260203_160608_create_tables;
mod m20260204_000001_repo_types_and_plugins;
mod m20260204_120001_create_auth_and_sso;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260203_160608_create_tables::Migration),
            Box::new(m20260204_000001_repo_types_and_plugins::Migration),
            Box::new(m20260204_120001_create_auth_and_sso::Migration),
        ]
    }
}
