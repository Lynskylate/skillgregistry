use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sso_identities")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub connection_id: Uuid,
    pub provider_user_id: String,
    pub user_id: Uuid,
    #[sea_orm(nullable)]
    pub email: Option<String>,
    pub email_verified: bool,
    #[sea_orm(nullable)]
    pub display_name: Option<String>,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sso_connections::Entity",
        from = "Column::ConnectionId",
        to = "super::sso_connections::Column::ConnectionId"
    )]
    SsoConnection,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::UserId"
    )]
    User,
}

impl Related<super::sso_connections::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SsoConnection.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
