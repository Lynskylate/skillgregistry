use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "refresh_tokens")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: Uuid,
    pub token_hash: String,
    #[sea_orm(nullable)]
    pub rotated_from: Option<i64>,
    pub expires_at: DateTime,
    #[sea_orm(nullable)]
    pub revoked_at: Option<DateTime>,
    #[sea_orm(nullable)]
    pub user_agent: Option<String>,
    #[sea_orm(nullable)]
    pub ip: Option<String>,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::UserId"
    )]
    User,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
