use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "task_logs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub task_name: String,
    pub status: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub details: Option<String>,
    pub started_at: DateTime,
    pub ended_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
