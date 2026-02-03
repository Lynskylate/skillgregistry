use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "organizations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub org_id: Uuid,
    pub name: String,
    #[sea_orm(unique)]
    pub slug: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::org_memberships::Entity")]
    Memberships,
    #[sea_orm(has_many = "super::sso_connections::Entity")]
    SsoConnections,
}

impl Related<super::org_memberships::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Memberships.def()
    }
}

impl Related<super::sso_connections::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SsoConnections.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
