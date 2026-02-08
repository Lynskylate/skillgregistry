pub mod auth_identities;
pub mod blacklist;
pub mod discovery_registries;
pub mod local_credentials;
pub mod org_memberships;
pub mod organizations;
pub mod plugin_components;
pub mod plugin_versions;
pub mod plugins;
pub mod prelude;
pub mod refresh_tokens;
pub mod skill_registry;
pub mod skill_versions;
pub mod skills;
pub mod sso_connections;
pub mod sso_identities;
pub mod task_logs;
pub mod users;

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Related;

    #[test]
    fn active_enum_serialization_roundtrip() {
        fn assert_roundtrip<T>(value: &T)
        where
            T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + core::fmt::Debug,
        {
            let encoded = serde_json::to_string(value).unwrap();
            let decoded: T = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, *value);
        }

        assert_roundtrip(&auth_identities::AuthProvider::Local);
        assert_roundtrip(&auth_identities::AuthProvider::Github);
        assert_roundtrip(&auth_identities::AuthProvider::Google);
        assert_roundtrip(&discovery_registries::Platform::Github);
        assert_roundtrip(&org_memberships::OrgRole::Owner);
        assert_roundtrip(&org_memberships::OrgRole::Admin);
        assert_roundtrip(&org_memberships::OrgRole::Member);
        assert_roundtrip(&skill_registry::Platform::Github);
        assert_roundtrip(&sso_connections::SsoProtocol::Oidc);
        assert_roundtrip(&sso_connections::SsoProtocol::Saml);
        assert_roundtrip(&users::UserStatus::Active);
        assert_roundtrip(&users::UserStatus::Disabled);
        assert_roundtrip(&users::UserRole::Admin);
        assert_roundtrip(&users::UserRole::User);
    }

    #[test]
    fn relation_definitions_are_accessible() {
        let _ = <auth_identities::Entity as Related<users::Entity>>::to();
        let _ = <discovery_registries::Entity as Related<skill_registry::Entity>>::to();
        let _ = <local_credentials::Entity as Related<users::Entity>>::to();
        let _ = <org_memberships::Entity as Related<organizations::Entity>>::to();
        let _ = <org_memberships::Entity as Related<users::Entity>>::to();
        let _ = <organizations::Entity as Related<org_memberships::Entity>>::to();
        let _ = <plugin_components::Entity as Related<plugin_versions::Entity>>::to();
        let _ = <plugin_versions::Entity as Related<plugins::Entity>>::to();
        let _ = <plugin_versions::Entity as Related<plugin_components::Entity>>::to();
        let _ = <plugins::Entity as Related<skill_registry::Entity>>::to();
        let _ = <plugins::Entity as Related<plugin_versions::Entity>>::to();
        let _ = <refresh_tokens::Entity as Related<users::Entity>>::to();
        let _ = <skill_registry::Entity as Related<skills::Entity>>::to();
        let _ = <skill_versions::Entity as Related<skills::Entity>>::to();
        let _ = <skills::Entity as Related<skill_registry::Entity>>::to();
        let _ = <skills::Entity as Related<skill_versions::Entity>>::to();
        let _ = <sso_connections::Entity as Related<organizations::Entity>>::to();
        let _ = <sso_connections::Entity as Related<sso_identities::Entity>>::to();
        let _ = <sso_identities::Entity as Related<sso_connections::Entity>>::to();
        let _ = <sso_identities::Entity as Related<users::Entity>>::to();
        let _ = <users::Entity as Related<auth_identities::Entity>>::to();
        let _ = <users::Entity as Related<local_credentials::Entity>>::to();
        let _ = <users::Entity as Related<refresh_tokens::Entity>>::to();
        let _ = <users::Entity as Related<org_memberships::Entity>>::to();
    }
}
