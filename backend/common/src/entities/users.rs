use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum UserStatus {
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "disabled")]
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum UserRole {
    #[sea_orm(string_value = "admin")]
    Admin,
    #[sea_orm(string_value = "user")]
    User,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: Uuid,
    pub status: UserStatus,
    pub role: UserRole,
    #[sea_orm(unique, nullable)]
    pub username: Option<String>,
    #[sea_orm(nullable)]
    pub display_name: Option<String>,
    #[sea_orm(unique, nullable)]
    pub primary_email: Option<String>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::auth_identities::Entity")]
    AuthIdentities,
    #[sea_orm(has_one = "super::local_credentials::Entity")]
    LocalCredentials,
    #[sea_orm(has_many = "super::refresh_tokens::Entity")]
    RefreshTokens,
    #[sea_orm(has_many = "super::org_memberships::Entity")]
    OrgMemberships,
}

impl Related<super::auth_identities::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AuthIdentities.def()
    }
}

impl Related<super::local_credentials::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LocalCredentials.def()
    }
}

impl Related<super::refresh_tokens::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RefreshTokens.def()
    }
}

impl Related<super::org_memberships::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OrgMemberships.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
