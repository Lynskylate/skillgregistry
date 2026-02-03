use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(256))")]
pub enum Platform {
    #[sea_orm(string_value = "github")]
    Github,
    #[sea_orm(string_value = "gitlab")]
    Gitlab,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "skill_registry")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub platform: Platform,
    pub owner: String,
    pub name: String,
    pub url: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    pub repo_type: Option<String>,
    pub status: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub blacklist_reason: Option<String>,
    pub blacklisted_at: Option<DateTime>,
    pub stars: i32,
    pub last_scanned_at: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::skills::Entity")]
    Skills,
}

impl Related<super::skills::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Skills.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
