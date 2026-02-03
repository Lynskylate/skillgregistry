use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum SsoProtocol {
    #[sea_orm(string_value = "oidc")]
    Oidc,
    #[sea_orm(string_value = "saml")]
    Saml,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sso_connections")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub connection_id: Uuid,
    pub org_id: Uuid,
    pub protocol: SsoProtocol,
    #[sea_orm(nullable)]
    pub issuer: Option<String>,
    #[sea_orm(nullable)]
    pub metadata_url: Option<String>,
    #[sea_orm(nullable)]
    pub sso_url: Option<String>,
    #[sea_orm(nullable)]
    pub x509_cert_fingerprint: Option<String>,
    #[sea_orm(nullable)]
    pub client_id: Option<String>,
    #[sea_orm(nullable)]
    pub client_secret: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub allowed_domains_json: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::organizations::Entity",
        from = "Column::OrgId",
        to = "super::organizations::Column::OrgId"
    )]
    Organization,
    #[sea_orm(has_many = "super::sso_identities::Entity")]
    SsoIdentities,
}

impl Related<super::organizations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Organization.def()
    }
}

impl Related<super::sso_identities::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SsoIdentities.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
