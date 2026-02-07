use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum Platform {
    #[sea_orm(string_value = "github")]
    Github,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "discovery_registries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub platform: Platform,
    #[sea_orm(column_type = "Text")]
    pub token: String,
    #[sea_orm(column_type = "Text")]
    pub api_url: String,
    #[sea_orm(column_type = "Text")]
    pub queries_json: String,
    pub schedule_interval_seconds: i64,
    pub last_health_status: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub last_health_message: Option<String>,
    pub last_health_checked_at: Option<DateTime>,
    pub last_run_at: Option<DateTime>,
    pub next_run_at: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::skill_registry::Entity")]
    SkillRegistry,
}

impl Related<super::skill_registry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SkillRegistry.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
