use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub identifier: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
}

#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SsoLookupRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct SsoLookupItem {
    pub connection_id: Uuid,
    pub org_id: Uuid,
    pub protocol: String,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user_id: Uuid,
    pub username: Option<String>,
    pub role: String,
    pub display_name: Option<String>,
    pub primary_email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct JwtClaims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub role: String,
    pub iat: i64,
    pub exp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct FlowCookiePayload {
    pub state: String,
    pub verifier: String,
    pub nonce: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OidcDiscovery {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub jwks_uri: String,
    pub issuer: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OidcTokenResponse {
    pub id_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OidcIdTokenClaims {
    pub iss: String,
    pub sub: String,
    pub aud: serde_json::Value,
    pub exp: i64,
    pub iat: i64,
    pub nonce: Option<String>,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
}
