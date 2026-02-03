use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "plugins")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub skill_registry_id: i32,
    pub name: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    #[sea_orm(column_type = "Json", nullable)]
    pub source: Option<serde_json::Value>,
    pub strict: i32,
    pub latest_version: Option<String>,
    pub is_active: i32,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::skill_registry::Entity",
        from = "Column::SkillRegistryId",
        to = "super::skill_registry::Column::Id"
    )]
    SkillRegistry,
    #[sea_orm(has_many = "super::plugin_versions::Entity")]
    Versions,
}

impl Related<super::skill_registry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SkillRegistry.def()
    }
}

impl Related<super::plugin_versions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Versions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
