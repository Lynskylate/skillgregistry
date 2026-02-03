use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "plugin_versions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub plugin_id: i32,
    pub version: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub readme_content: Option<String>,
    pub s3_key: Option<String>,
    pub oss_url: Option<String>,
    pub file_hash: Option<String>,
    #[sea_orm(column_type = "Json", nullable)]
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::plugins::Entity",
        from = "Column::PluginId",
        to = "super::plugins::Column::Id"
    )]
    Plugin,
    #[sea_orm(has_many = "super::plugin_components::Entity")]
    Components,
}

impl Related<super::plugins::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Plugin.def()
    }
}

impl Related<super::plugin_components::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Components.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
