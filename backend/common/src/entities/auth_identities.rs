use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(64))")]
pub enum AuthProvider {
    #[sea_orm(string_value = "local")]
    Local,
    #[sea_orm(string_value = "github")]
    Github,
    #[sea_orm(string_value = "google")]
    Google,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "auth_identities")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: Uuid,
    pub provider: AuthProvider,
    pub provider_user_id: String,
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
