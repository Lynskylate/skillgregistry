pub use sea_orm_migration::prelude::*;

mod m20260205_000001_create_all_tables;
mod m20260207_000002_add_api_url_to_discovery_registries;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260205_000001_create_all_tables::Migration),
            Box::new(m20260207_000002_add_api_url_to_discovery_registries::Migration),
        ]
    }
}
