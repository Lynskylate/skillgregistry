use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "plugin_components")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub plugin_version_id: i32,
    pub kind: String,
    pub path: String,
    pub name: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub markdown_content: Option<String>,
    #[sea_orm(column_type = "Json", nullable)]
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::plugin_versions::Entity",
        from = "Column::PluginVersionId",
        to = "super::plugin_versions::Column::Id"
    )]
    PluginVersion,
}

impl Related<super::plugin_versions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PluginVersion.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
