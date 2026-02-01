use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "skill_versions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub skill_id: i32,
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
    #[sea_orm(belongs_to = "super::skills::Entity", from = "Column::SkillId", to = "super::skills::Column::Id")]
    Skill,
}

impl Related<super::skills::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Skill.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
