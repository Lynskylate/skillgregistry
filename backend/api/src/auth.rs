use crate::models::ApiResponse;
use crate::origin::{is_origin_allowed as origin_matches, parse_frontend_origins};
use crate::AppState;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    async_trait,
    extract::{FromRequestParts, Path, Query, State},
    http::{
        header::{AUTHORIZATION, ORIGIN},
        request::Parts,
        HeaderMap, StatusCode,
    },
    response::{IntoResponse, Redirect},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{Duration, Utc};
use common::entities::prelude::{
    AuthIdentities, LocalCredentials, RefreshTokens, SsoConnections, SsoIdentities, Users,
};
use common::entities::{
    auth_identities, local_credentials, org_memberships, refresh_tokens, sso_connections,
    sso_identities, users,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::{rngs::OsRng, RngCore};
use reqwest::Client;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, IntoActiveModel, QueryFilter, Set,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use url::Url;
use uuid::Uuid;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
        .route("/oauth/:provider/start", get(oauth_start))
        .route("/oauth/:provider/callback", get(oauth_callback))
        .route("/sso/:connection_id/start", get(sso_start))
        .route("/sso/:connection_id/callback", get(sso_callback))
        .route("/sso/:connection_id/acs", post(sso_acs))
        .route("/sso/:connection_id/metadata", get(sso_metadata))
        .route("/sso/lookup", post(sso_lookup))
}

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
struct JwtClaims {
    iss: String,
    aud: String,
    sub: String,
    role: String,
    iat: i64,
    exp: i64,
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub role: String,
}

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = (StatusCode, Json<ApiResponse<()>>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let token = auth_header
            .strip_prefix("Bearer ")
            .or_else(|| auth_header.strip_prefix("bearer "))
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(ApiResponse::error(401, "missing bearer token".to_string())),
                )
            })?;

        let signing_key = state.settings.auth.jwt.signing_key.clone().ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(
                    500,
                    "jwt signing key not configured".to_string(),
                )),
            )
        })?;

        let mut validation = Validation::default();
        validation.set_issuer(std::slice::from_ref(&state.settings.auth.jwt.issuer));
        validation.set_audience(std::slice::from_ref(&state.settings.auth.jwt.audience));

        let decoded = decode::<JwtClaims>(
            token,
            &DecodingKey::from_secret(signing_key.as_bytes()),
            &validation,
        )
        .map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::error(401, "invalid token".to_string())),
            )
        })?;

        let user_id = Uuid::parse_str(&decoded.claims.sub).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::error(401, "invalid token sub".to_string())),
            )
        })?;

        Ok(Self {
            user_id,
            role: decoded.claims.role,
        })
    }
}

async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> (CookieJar, Json<ApiResponse<LoginResponse>>) {
    let db = state.db.as_ref();
    let now = Utc::now().naive_utc();

    let username = req.username.trim().to_lowercase();
    if username.is_empty() || req.password.is_empty() {
        return (
            CookieJar::new(),
            Json(ApiResponse::error(400, "invalid input".to_string())),
        );
    }

    if let Ok(Some(_)) = Users::find()
        .filter(users::Column::Username.eq(username.clone()))
        .one(db)
        .await
    {
        return (
            CookieJar::new(),
            Json(ApiResponse::error(
                409,
                "username already exists".to_string(),
            )),
        );
    }

    if let Some(email) = req.email.as_ref().map(|e| e.trim().to_lowercase()) {
        if !email.is_empty() {
            if let Ok(Some(_)) = Users::find()
                .filter(users::Column::PrimaryEmail.eq(email))
                .one(db)
                .await
            {
                return (
                    CookieJar::new(),
                    Json(ApiResponse::error(409, "email already exists".to_string())),
                );
            }
        }
    }

    let password_hash = match hash_password(&req.password) {
        Ok(h) => h,
        Err(_) => {
            return (
                CookieJar::new(),
                Json(ApiResponse::error(
                    500,
                    "failed to hash password".to_string(),
                )),
            )
        }
    };

    let user_id = Uuid::new_v4();
    let user_am = users::ActiveModel {
        user_id: Set(user_id),
        status: Set(users::UserStatus::Active),
        role: Set(users::UserRole::User),
        username: Set(Some(username.clone())),
        display_name: Set(req.display_name.clone()),
        primary_email: Set(req
            .email
            .as_ref()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())),
        created_at: Set(now),
        updated_at: Set(now),
    };

    if user_am.insert(db).await.is_err() {
        return (
            CookieJar::new(),
            Json(ApiResponse::error(500, "failed to create user".to_string())),
        );
    }

    let identity_am = auth_identities::ActiveModel {
        user_id: Set(user_id),
        provider: Set(auth_identities::AuthProvider::Local),
        provider_user_id: Set(username),
        email: Set(req
            .email
            .as_ref()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())),
        email_verified: Set(false),
        display_name: Set(req.display_name.clone()),
        created_at: Set(now),
        ..Default::default()
    };

    if identity_am.insert(db).await.is_err() {
        return (
            CookieJar::new(),
            Json(ApiResponse::error(
                500,
                "failed to create identity".to_string(),
            )),
        );
    }

    let cred_am = local_credentials::ActiveModel {
        user_id: Set(user_id),
        password_hash: Set(password_hash),
        password_updated_at: Set(now),
    };

    if cred_am.insert(db).await.is_err() {
        return (
            CookieJar::new(),
            Json(ApiResponse::error(
                500,
                "failed to create credentials".to_string(),
            )),
        );
    }

    issue_tokens_and_set_cookie(&state, user_id, "user".to_string(), None).await
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> (CookieJar, Json<ApiResponse<LoginResponse>>) {
    let db = &*state.db;
    let identifier = req.identifier.trim().to_lowercase();
    if identifier.is_empty() || req.password.is_empty() {
        return (
            CookieJar::new(),
            Json(ApiResponse::error(400, "invalid input".to_string())),
        );
    }

    let user = match Users::find()
        .filter(
            Condition::any()
                .add(users::Column::Username.eq(identifier.clone()))
                .add(users::Column::PrimaryEmail.eq(identifier.clone())),
        )
        .one(db)
        .await
    {
        Ok(Some(u)) => u,
        _ => {
            return (
                CookieJar::new(),
                Json(ApiResponse::error(401, "invalid credentials".to_string())),
            )
        }
    };

    if user.status != users::UserStatus::Active {
        return (
            CookieJar::new(),
            Json(ApiResponse::error(403, "user disabled".to_string())),
        );
    }

    let cred = match LocalCredentials::find_by_id(user.user_id).one(db).await {
        Ok(Some(c)) => c,
        _ => {
            return (
                CookieJar::new(),
                Json(ApiResponse::error(401, "invalid credentials".to_string())),
            )
        }
    };

    if verify_password(&req.password, &cred.password_hash).is_err() {
        return (
            CookieJar::new(),
            Json(ApiResponse::error(401, "invalid credentials".to_string())),
        );
    }

    let role = match user.role {
        users::UserRole::Admin => "admin",
        users::UserRole::User => "user",
    }
    .to_string();

    issue_tokens_and_set_cookie(&state, user.user_id, role, None).await
}

async fn refresh(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    jar: CookieJar,
) -> (CookieJar, Json<ApiResponse<LoginResponse>>) {
    if !origin_allowed(&state, &headers) {
        return (
            jar,
            Json(ApiResponse::error(403, "origin not allowed".to_string())),
        );
    }

    let db = state.db.as_ref();
    let now = Utc::now().naive_utc();
    let cookie = match jar.get(REFRESH_COOKIE_NAME) {
        Some(c) => c,
        None => {
            return (
                jar,
                Json(ApiResponse::error(401, "missing refresh token".to_string())),
            )
        }
    };

    let token_hash = sha256_hex(cookie.value());
    let token = match RefreshTokens::find()
        .filter(refresh_tokens::Column::TokenHash.eq(token_hash))
        .filter(refresh_tokens::Column::RevokedAt.is_null())
        .filter(refresh_tokens::Column::ExpiresAt.gt(now))
        .one(db)
        .await
    {
        Ok(Some(t)) => t,
        _ => {
            return (
                jar,
                Json(ApiResponse::error(401, "invalid refresh token".to_string())),
            )
        }
    };

    let user = match Users::find_by_id(token.user_id).one(db).await {
        Ok(Some(u)) => u,
        _ => {
            return (
                jar,
                Json(ApiResponse::error(401, "invalid refresh token".to_string())),
            )
        }
    };

    if user.status != users::UserStatus::Active {
        return (
            jar,
            Json(ApiResponse::error(403, "user disabled".to_string())),
        );
    }

    let role = match user.role {
        users::UserRole::Admin => "admin",
        users::UserRole::User => "user",
    }
    .to_string();

    issue_tokens_and_set_cookie(&state, user.user_id, role, Some(token.id)).await
}

async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    jar: CookieJar,
) -> (CookieJar, Json<ApiResponse<()>>) {
    if !origin_allowed(&state, &headers) {
        return (
            jar,
            Json(ApiResponse::error(403, "origin not allowed".to_string())),
        );
    }

    let db = state.db.as_ref();
    let now = Utc::now().naive_utc();

    if let Some(cookie) = jar.get(REFRESH_COOKIE_NAME) {
        let token_hash = sha256_hex(cookie.value());
        if let Ok(Some(token)) = RefreshTokens::find()
            .filter(refresh_tokens::Column::TokenHash.eq(token_hash))
            .filter(refresh_tokens::Column::RevokedAt.is_null())
            .one(db)
            .await
        {
            let mut am: refresh_tokens::ActiveModel = token.into_active_model();
            am.revoked_at = Set(Some(now));
            let _ = am.update(db).await;
        }
    }

    let cleared = clear_refresh_cookie(&state);
    let jar = jar.remove(cleared);
    (jar, Json(ApiResponse::success(())))
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> Json<ApiResponse<MeResponse>> {
    let db = state.db.as_ref();
    let model = match Users::find_by_id(user.user_id).one(db).await {
        Ok(Some(u)) => u,
        _ => return Json(ApiResponse::error(404, "user not found".to_string())),
    };

    Json(ApiResponse::success(MeResponse {
        user_id: model.user_id,
        username: model.username,
        role: user.role,
        display_name: model.display_name,
        primary_email: model.primary_email,
    }))
}

async fn sso_lookup(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SsoLookupRequest>,
) -> Json<ApiResponse<Vec<SsoLookupItem>>> {
    let db = state.db.as_ref();
    let email = req.email.trim().to_lowercase();
    let domain = match email.split('@').nth(1) {
        Some(d) if !d.is_empty() => d.to_string(),
        _ => return Json(ApiResponse::error(400, "invalid email".to_string())),
    };

    let connections = match SsoConnections::find()
        .filter(sso_connections::Column::Enabled.eq(true))
        .all(db)
        .await
    {
        Ok(v) => v,
        Err(e) => return Json(ApiResponse::error(500, e.to_string())),
    };

    let mut matches = Vec::new();
    for c in connections {
        let allowed: Vec<String> = c
            .allowed_domains_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|d| d.to_lowercase())
            .collect();

        if allowed.iter().any(|d| d == &domain) {
            matches.push(SsoLookupItem {
                connection_id: c.connection_id,
                org_id: c.org_id,
                protocol: match c.protocol {
                    common::entities::sso_connections::SsoProtocol::Oidc => "oidc",
                    common::entities::sso_connections::SsoProtocol::Saml => "saml",
                }
                .to_string(),
            });
        }
    }

    Json(ApiResponse::success(matches))
}

async fn oauth_start(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
) -> impl IntoResponse {
    let provider = provider.trim().to_lowercase();
    let (client_id, redirect_uri, scopes, auth_url) = match provider.as_str() {
        "github" => {
            let cfg = match state.settings.auth.oauth.github.as_ref() {
                Some(c) => c,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ApiResponse::<()>::error(
                            400,
                            "oauth provider not configured".to_string(),
                        )),
                    )
                        .into_response()
                }
            };
            let scopes = if cfg.scopes.is_empty() {
                vec!["read:user".to_string(), "user:email".to_string()]
            } else {
                cfg.scopes.clone()
            };
            (
                cfg.client_id.clone(),
                cfg.redirect_url.clone(),
                scopes,
                "https://github.com/login/oauth/authorize".to_string(),
            )
        }
        "google" => {
            let cfg = match state.settings.auth.oauth.google.as_ref() {
                Some(c) => c,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ApiResponse::<()>::error(
                            400,
                            "oauth provider not configured".to_string(),
                        )),
                    )
                        .into_response()
                }
            };
            let scopes = if cfg.scopes.is_empty() {
                vec![
                    "openid".to_string(),
                    "email".to_string(),
                    "profile".to_string(),
                ]
            } else {
                cfg.scopes.clone()
            };
            (
                cfg.client_id.clone(),
                cfg.redirect_url.clone(),
                scopes,
                "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            )
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "unknown oauth provider".to_string(),
                )),
            )
                .into_response()
        }
    };

    let state_value = random_token();
    let verifier = random_token();
    let nonce = if provider == "google" {
        Some(random_token())
    } else {
        None
    };

    let cookie_payload = match serde_json::to_vec(&FlowCookiePayload {
        state: state_value.clone(),
        verifier: verifier.clone(),
        nonce: nonce.clone(),
    }) {
        Ok(v) => URL_SAFE_NO_PAD.encode(v),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(
                    500,
                    "failed to build oauth state".to_string(),
                )),
            )
                .into_response()
        }
    };

    let cookie_name = format!("sr_oauth_{}", provider);
    let cookie = build_flow_cookie(
        &state,
        &cookie_name,
        &cookie_payload,
        &format!("/api/auth/oauth/{}", provider),
    );

    let challenge = pkce_challenge(&verifier);
    let mut url = match Url::parse(&auth_url) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(
                    500,
                    "invalid auth url".to_string(),
                )),
            )
                .into_response()
        }
    };

    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("client_id", &client_id);
        qp.append_pair("redirect_uri", &redirect_uri);
        qp.append_pair("response_type", "code");
        qp.append_pair("scope", &scopes.join(" "));
        qp.append_pair("state", &state_value);
        qp.append_pair("code_challenge_method", "S256");
        qp.append_pair("code_challenge", &challenge);
        if let Some(nonce) = nonce.as_ref() {
            qp.append_pair("nonce", nonce);
        }
    }

    (
        CookieJar::new().add(cookie),
        Redirect::temporary(url.as_str()),
    )
        .into_response()
}

async fn oauth_callback(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Query(q): Query<OAuthCallbackQuery>,
    jar: CookieJar,
) -> impl IntoResponse {
    let provider = provider.trim().to_lowercase();
    let code = match q.code.as_ref().map(|s| s.trim().to_string()) {
        Some(c) if !c.is_empty() => c,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(400, "missing code".to_string())),
            )
                .into_response()
        }
    };
    let state_param = match q.state.as_ref().map(|s| s.trim().to_string()) {
        Some(s) if !s.is_empty() => s,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(400, "missing state".to_string())),
            )
                .into_response()
        }
    };

    let cookie_name = format!("sr_oauth_{}", provider);
    let flow = match read_flow_cookie(&jar, &cookie_name) {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "invalid oauth state".to_string(),
                )),
            )
                .into_response()
        }
    };

    if flow.state != state_param {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(400, "state mismatch".to_string())),
        )
            .into_response();
    }

    let client = Client::new();
    match provider.as_str() {
        "github" => oauth_callback_github(&state, client, code, flow.verifier, jar).await,
        "google" => oauth_callback_google(&state, client, code, flow, jar).await,
        _ => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                400,
                "unknown oauth provider".to_string(),
            )),
        )
            .into_response(),
    }
}

async fn sso_start(
    State(state): State<Arc<AppState>>,
    Path(connection_id): Path<Uuid>,
) -> impl IntoResponse {
    let db = state.db.as_ref();
    let conn = match SsoConnections::find_by_id(connection_id).one(db).await {
        Ok(Some(c)) => c,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()>::error(
                    404,
                    "sso connection not found".to_string(),
                )),
            )
                .into_response()
        }
    };

    if !conn.enabled || conn.protocol != sso_connections::SsoProtocol::Oidc {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                400,
                "sso connection not enabled".to_string(),
            )),
        )
            .into_response();
    }

    let issuer_hint = conn.issuer.clone();
    let metadata_url = conn.metadata_url.clone().or_else(|| {
        issuer_hint.as_ref().map(|iss| {
            format!(
                "{}/.well-known/openid-configuration",
                iss.trim_end_matches('/')
            )
        })
    });

    let client_id = match conn.client_id.clone() {
        Some(v) if !v.is_empty() => v,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "missing client_id".to_string(),
                )),
            )
                .into_response()
        }
    };

    let redirect_uri = build_sso_callback_url(&state, connection_id);
    let state_value = random_token();
    let verifier = random_token();
    let nonce = Some(random_token());

    let cookie_payload = match serde_json::to_vec(&FlowCookiePayload {
        state: state_value.clone(),
        verifier: verifier.clone(),
        nonce: nonce.clone(),
    }) {
        Ok(v) => URL_SAFE_NO_PAD.encode(v),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(
                    500,
                    "failed to build sso state".to_string(),
                )),
            )
                .into_response()
        }
    };

    let cookie_name = format!("sr_sso_{}", connection_id);
    let cookie = build_flow_cookie(
        &state,
        &cookie_name,
        &cookie_payload,
        &format!("/api/auth/sso/{}", connection_id),
    );

    let client = Client::new();
    let discovery = match fetch_oidc_discovery(&client, metadata_url.as_deref()).await {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(400, e)),
            )
                .into_response()
        }
    };

    let auth_endpoint = discovery.authorization_endpoint;
    let challenge = pkce_challenge(&verifier);
    let mut url = match Url::parse(&auth_endpoint) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(
                    500,
                    "invalid authorization endpoint".to_string(),
                )),
            )
                .into_response()
        }
    };

    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("client_id", &client_id);
        qp.append_pair("redirect_uri", &redirect_uri);
        qp.append_pair("response_type", "code");
        qp.append_pair("scope", "openid email profile");
        qp.append_pair("state", &state_value);
        qp.append_pair("code_challenge_method", "S256");
        qp.append_pair("code_challenge", &challenge);
        if let Some(nonce) = nonce.as_ref() {
            qp.append_pair("nonce", nonce);
        }
    }

    (
        CookieJar::new().add(cookie),
        Redirect::temporary(url.as_str()),
    )
        .into_response()
}

async fn sso_callback(
    State(state): State<Arc<AppState>>,
    Path(connection_id): Path<Uuid>,
    Query(q): Query<OAuthCallbackQuery>,
    jar: CookieJar,
) -> impl IntoResponse {
    let code = match q.code.as_ref().map(|s| s.trim().to_string()) {
        Some(c) if !c.is_empty() => c,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(400, "missing code".to_string())),
            )
                .into_response()
        }
    };
    let state_param = match q.state.as_ref().map(|s| s.trim().to_string()) {
        Some(s) if !s.is_empty() => s,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(400, "missing state".to_string())),
            )
                .into_response()
        }
    };

    let cookie_name = format!("sr_sso_{}", connection_id);
    let flow = match read_flow_cookie(&jar, &cookie_name) {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "invalid sso state".to_string(),
                )),
            )
                .into_response()
        }
    };

    if flow.state != state_param {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(400, "state mismatch".to_string())),
        )
            .into_response();
    }

    let db = state.db.as_ref();
    let conn = match SsoConnections::find_by_id(connection_id).one(db).await {
        Ok(Some(c)) => c,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()>::error(
                    404,
                    "sso connection not found".to_string(),
                )),
            )
                .into_response()
        }
    };

    if !conn.enabled || conn.protocol != sso_connections::SsoProtocol::Oidc {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                400,
                "sso connection not enabled".to_string(),
            )),
        )
            .into_response();
    }

    let client_id = match conn.client_id.clone() {
        Some(v) if !v.is_empty() => v,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "missing client_id".to_string(),
                )),
            )
                .into_response()
        }
    };
    let client_secret = conn.client_secret.clone();
    let redirect_uri = build_sso_callback_url(&state, connection_id);

    let issuer_hint = conn.issuer.clone();
    let metadata_url = conn.metadata_url.clone().or_else(|| {
        issuer_hint.as_ref().map(|iss| {
            format!(
                "{}/.well-known/openid-configuration",
                iss.trim_end_matches('/')
            )
        })
    });

    let client = Client::new();
    let discovery = match fetch_oidc_discovery(&client, metadata_url.as_deref()).await {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(400, e)),
            )
                .into_response()
        }
    };

    let token_resp = match exchange_code_for_token(
        &client,
        &discovery.token_endpoint,
        &client_id,
        client_secret.as_deref(),
        &redirect_uri,
        &code,
        &flow.verifier,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(400, e)),
            )
                .into_response()
        }
    };

    let id_token = match token_resp.id_token {
        Some(t) if !t.is_empty() => t,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "missing id_token".to_string(),
                )),
            )
                .into_response()
        }
    };

    let claims = match verify_oidc_id_token(
        &client,
        &discovery.jwks_uri,
        &id_token,
        conn.issuer.as_deref().unwrap_or(&discovery.issuer),
        &client_id,
        flow.nonce.as_deref(),
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(400, e)),
            )
                .into_response()
        }
    };

    let (jar, redirect) =
        match login_or_create_user_for_sso(&state, connection_id, conn.org_id, claims).await {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<()>::error(500, e)),
                )
                    .into_response()
            }
        };

    (jar, redirect).into_response()
}

async fn sso_acs(
    State(_state): State<Arc<AppState>>,
    Path(_connection_id): Path<Uuid>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiResponse::<()>::error(
            501,
            "saml not implemented yet".to_string(),
        )),
    )
        .into_response()
}

async fn sso_metadata(
    State(_state): State<Arc<AppState>>,
    Path(_connection_id): Path<Uuid>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiResponse::<()>::error(
            501,
            "saml not implemented yet".to_string(),
        )),
    )
        .into_response()
}

#[derive(Debug, Serialize, Deserialize)]
struct FlowCookiePayload {
    state: String,
    verifier: String,
    nonce: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OidcDiscovery {
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    issuer: String,
}

#[derive(Debug, Deserialize)]
struct OidcTokenResponse {
    id_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OidcIdTokenClaims {
    iss: String,
    sub: String,
    aud: serde_json::Value,
    exp: i64,
    iat: i64,
    nonce: Option<String>,
    email: Option<String>,
    email_verified: Option<bool>,
    name: Option<String>,
}

async fn oauth_callback_github(
    state: &Arc<AppState>,
    client: Client,
    code: String,
    verifier: String,
    _jar: CookieJar,
) -> axum::response::Response {
    let cfg = match state.settings.auth.oauth.github.as_ref() {
        Some(c) => c,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "oauth provider not configured".to_string(),
                )),
            )
                .into_response()
        }
    };

    let token_resp = match client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&serde_json::json!({
            "client_id": cfg.client_id,
            "client_secret": cfg.client_secret,
            "code": code,
            "redirect_uri": cfg.redirect_url,
            "code_verifier": verifier,
        }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "token exchange failed".to_string(),
                )),
            )
                .into_response()
        }
    };

    let token_json: serde_json::Value = match token_resp.json().await {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "invalid token response".to_string(),
                )),
            )
                .into_response()
        }
    };

    let access = match token_json
        .get("access_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    {
        Some(v) if !v.is_empty() => v,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "missing access_token".to_string(),
                )),
            )
                .into_response()
        }
    };

    let user_resp = match client
        .get("https://api.github.com/user")
        .header("User-Agent", "skillregistry")
        .bearer_auth(access)
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "github user fetch failed".to_string(),
                )),
            )
                .into_response()
        }
    };

    let user_json: serde_json::Value = match user_resp.json().await {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "invalid github user response".to_string(),
                )),
            )
                .into_response()
        }
    };

    let github_id = match user_json.get("id").and_then(|v| v.as_u64()) {
        Some(v) => v.to_string(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "missing github id".to_string(),
                )),
            )
                .into_response()
        }
    };

    let login = user_json
        .get("login")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let name = user_json
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let db = state.db.as_ref();
    let now = Utc::now().naive_utc();

    let existing = AuthIdentities::find()
        .filter(auth_identities::Column::Provider.eq(auth_identities::AuthProvider::Github))
        .filter(auth_identities::Column::ProviderUserId.eq(github_id.clone()))
        .one(db)
        .await;

    let (user_id, role) = match existing {
        Ok(Some(identity)) => {
            let user = match Users::find_by_id(identity.user_id).one(db).await {
                Ok(Some(u)) => u,
                _ => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ApiResponse::<()>::error(400, "user not found".to_string())),
                    )
                        .into_response()
                }
            };
            let role = match user.role {
                users::UserRole::Admin => "admin",
                users::UserRole::User => "user",
            }
            .to_string();
            (user.user_id, role)
        }
        _ => {
            let username = login
                .as_ref()
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty());

            let username = if let Some(u) = username {
                match Users::find()
                    .filter(users::Column::Username.eq(u.clone()))
                    .one(db)
                    .await
                {
                    Ok(Some(_)) => None,
                    _ => Some(u),
                }
            } else {
                None
            };

            let user_id = Uuid::new_v4();
            let user_am = users::ActiveModel {
                user_id: Set(user_id),
                status: Set(users::UserStatus::Active),
                role: Set(users::UserRole::User),
                username: Set(username),
                display_name: Set(name.clone().or(login.clone())),
                primary_email: Set(None),
                created_at: Set(now),
                updated_at: Set(now),
            };

            if user_am.insert(db).await.is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<()>::error(
                        500,
                        "failed to create user".to_string(),
                    )),
                )
                    .into_response();
            }

            let identity_am = auth_identities::ActiveModel {
                user_id: Set(user_id),
                provider: Set(auth_identities::AuthProvider::Github),
                provider_user_id: Set(github_id),
                email: Set(None),
                email_verified: Set(false),
                display_name: Set(name.or(login)),
                created_at: Set(now),
                ..Default::default()
            };

            if identity_am.insert(db).await.is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<()>::error(
                        500,
                        "failed to create identity".to_string(),
                    )),
                )
                    .into_response();
            }

            (user_id, "user".to_string())
        }
    };

    let (jar2, _json) = issue_tokens_and_set_cookie(state, user_id, role, None).await;
    let jar2 = jar2.remove(clear_named_cookie(
        state,
        "sr_oauth_github",
        "/api/auth/oauth/github",
    ));
    let redirect = Redirect::temporary(&frontend_post_auth_url(state));
    (jar2, redirect).into_response()
}

async fn oauth_callback_google(
    state: &Arc<AppState>,
    client: Client,
    code: String,
    flow: FlowCookiePayload,
    _jar: CookieJar,
) -> axum::response::Response {
    let cfg = match state.settings.auth.oauth.google.as_ref() {
        Some(c) => c,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "oauth provider not configured".to_string(),
                )),
            )
                .into_response()
        }
    };

    let discovery = match fetch_oidc_discovery(
        &client,
        Some("https://accounts.google.com/.well-known/openid-configuration"),
    )
    .await
    {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(400, e)),
            )
                .into_response()
        }
    };

    let token_resp = match exchange_code_for_token(
        &client,
        &discovery.token_endpoint,
        &cfg.client_id,
        Some(&cfg.client_secret),
        &cfg.redirect_url,
        &code,
        &flow.verifier,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(400, e)),
            )
                .into_response()
        }
    };

    let id_token = match token_resp.id_token {
        Some(t) if !t.is_empty() => t,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    400,
                    "missing id_token".to_string(),
                )),
            )
                .into_response()
        }
    };

    let claims = match verify_oidc_id_token(
        &client,
        &discovery.jwks_uri,
        &id_token,
        &discovery.issuer,
        &cfg.client_id,
        flow.nonce.as_deref(),
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(400, e)),
            )
                .into_response()
        }
    };

    let db = state.db.as_ref();
    let now = Utc::now().naive_utc();

    let existing = AuthIdentities::find()
        .filter(auth_identities::Column::Provider.eq(auth_identities::AuthProvider::Google))
        .filter(auth_identities::Column::ProviderUserId.eq(claims.sub.clone()))
        .one(db)
        .await;

    let (user_id, role) = match existing {
        Ok(Some(identity)) => {
            let user = match Users::find_by_id(identity.user_id).one(db).await {
                Ok(Some(u)) => u,
                _ => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ApiResponse::<()>::error(400, "user not found".to_string())),
                    )
                        .into_response()
                }
            };
            let role = match user.role {
                users::UserRole::Admin => "admin",
                users::UserRole::User => "user",
            }
            .to_string();
            (user.user_id, role)
        }
        _ => {
            let email_verified = claims.email_verified.unwrap_or(false);
            let email = claims
                .email
                .as_ref()
                .map(|e| e.trim().to_lowercase())
                .filter(|e| !e.is_empty());

            let primary_email = if email_verified {
                if let Some(e) = email.clone() {
                    match Users::find()
                        .filter(users::Column::PrimaryEmail.eq(e.clone()))
                        .one(db)
                        .await
                    {
                        Ok(Some(_)) => None,
                        _ => Some(e),
                    }
                } else {
                    None
                }
            } else {
                None
            };

            let user_id = Uuid::new_v4();
            let user_am = users::ActiveModel {
                user_id: Set(user_id),
                status: Set(users::UserStatus::Active),
                role: Set(users::UserRole::User),
                username: Set(None),
                display_name: Set(claims.name.clone()),
                primary_email: Set(primary_email.clone()),
                created_at: Set(now),
                updated_at: Set(now),
            };

            if user_am.insert(db).await.is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<()>::error(
                        500,
                        "failed to create user".to_string(),
                    )),
                )
                    .into_response();
            }

            let identity_am = auth_identities::ActiveModel {
                user_id: Set(user_id),
                provider: Set(auth_identities::AuthProvider::Google),
                provider_user_id: Set(claims.sub),
                email: Set(email),
                email_verified: Set(email_verified),
                display_name: Set(claims.name),
                created_at: Set(now),
                ..Default::default()
            };

            if identity_am.insert(db).await.is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<()>::error(
                        500,
                        "failed to create identity".to_string(),
                    )),
                )
                    .into_response();
            }

            (user_id, "user".to_string())
        }
    };

    let (jar2, _json) = issue_tokens_and_set_cookie(state, user_id, role, None).await;
    let jar2 = jar2.remove(clear_named_cookie(
        state,
        "sr_oauth_google",
        "/api/auth/oauth/google",
    ));
    let redirect = Redirect::temporary(&frontend_post_auth_url(state));
    (jar2, redirect).into_response()
}

async fn login_or_create_user_for_sso(
    state: &Arc<AppState>,
    connection_id: Uuid,
    org_id: Uuid,
    claims: OidcIdTokenClaims,
) -> Result<(CookieJar, Redirect), String> {
    let db = state.db.as_ref();
    let now = Utc::now().naive_utc();

    let existing = SsoIdentities::find()
        .filter(sso_identities::Column::ConnectionId.eq(connection_id))
        .filter(sso_identities::Column::ProviderUserId.eq(claims.sub.clone()))
        .one(db)
        .await
        .map_err(|e| e.to_string())?;

    let (user_id, role) = if let Some(identity) = existing {
        let user = Users::find_by_id(identity.user_id)
            .one(db)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "user not found".to_string())?;
        let role = match user.role {
            users::UserRole::Admin => "admin",
            users::UserRole::User => "user",
        }
        .to_string();
        (user.user_id, role)
    } else {
        let email_verified = claims.email_verified.unwrap_or(false);
        let email = claims
            .email
            .as_ref()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty());

        let primary_email = if email_verified {
            if let Some(e) = email.clone() {
                match Users::find()
                    .filter(users::Column::PrimaryEmail.eq(e.clone()))
                    .one(db)
                    .await
                {
                    Ok(Some(_)) => None,
                    _ => Some(e),
                }
            } else {
                None
            }
        } else {
            None
        };

        let user_id = Uuid::new_v4();
        users::ActiveModel {
            user_id: Set(user_id),
            status: Set(users::UserStatus::Active),
            role: Set(users::UserRole::User),
            username: Set(None),
            display_name: Set(claims.name.clone()),
            primary_email: Set(primary_email),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .map_err(|e| e.to_string())?;

        sso_identities::ActiveModel {
            connection_id: Set(connection_id),
            provider_user_id: Set(claims.sub),
            user_id: Set(user_id),
            email: Set(email),
            email_verified: Set(email_verified),
            display_name: Set(claims.name),
            created_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|e| e.to_string())?;

        let _ = org_memberships::ActiveModel {
            org_id: Set(org_id),
            user_id: Set(user_id),
            org_role: Set(org_memberships::OrgRole::Member),
            created_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await;

        (user_id, "user".to_string())
    };

    let (jar, _json) = issue_tokens_and_set_cookie(state, user_id, role, None).await;
    Ok((jar, Redirect::temporary(&frontend_post_auth_url(state))))
}

async fn fetch_oidc_discovery(
    client: &Client,
    metadata_url: Option<&str>,
) -> Result<OidcDiscovery, String> {
    let url = metadata_url.ok_or_else(|| "missing oidc metadata url".to_string())?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|_| "failed to fetch oidc discovery".to_string())?;
    resp.json::<OidcDiscovery>()
        .await
        .map_err(|_| "invalid oidc discovery".to_string())
}

async fn exchange_code_for_token(
    client: &Client,
    token_endpoint: &str,
    client_id: &str,
    client_secret: Option<&str>,
    redirect_uri: &str,
    code: &str,
    verifier: &str,
) -> Result<OidcTokenResponse, String> {
    let mut form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code.to_string()),
        ("redirect_uri", redirect_uri.to_string()),
        ("client_id", client_id.to_string()),
        ("code_verifier", verifier.to_string()),
    ];
    if let Some(secret) = client_secret {
        form.push(("client_secret", secret.to_string()));
    }
    let resp = client
        .post(token_endpoint)
        .form(&form)
        .send()
        .await
        .map_err(|_| "token exchange failed".to_string())?;
    resp.json::<OidcTokenResponse>()
        .await
        .map_err(|_| "invalid token response".to_string())
}

async fn verify_oidc_id_token(
    client: &Client,
    jwks_uri: &str,
    id_token: &str,
    issuer: &str,
    client_id: &str,
    expected_nonce: Option<&str>,
) -> Result<OidcIdTokenClaims, String> {
    let jwks = client
        .get(jwks_uri)
        .send()
        .await
        .map_err(|_| "failed to fetch jwks".to_string())?
        .json::<jsonwebtoken::jwk::JwkSet>()
        .await
        .map_err(|_| "invalid jwks".to_string())?;

    let header =
        jsonwebtoken::decode_header(id_token).map_err(|_| "invalid id_token".to_string())?;
    let kid = header.kid.ok_or_else(|| "missing kid".to_string())?;

    let jwk = jwks
        .keys
        .iter()
        .find(|k| k.common.key_id.as_deref() == Some(&kid))
        .ok_or_else(|| "kid not found".to_string())?;

    let decoding_key =
        DecodingKey::from_jwk(jwk).map_err(|_| "failed to build decoding key".to_string())?;

    let mut validation = Validation::new(header.alg);
    validation.validate_aud = false;
    validation.set_issuer(&[issuer.to_string()]);

    let decoded = decode::<OidcIdTokenClaims>(id_token, &decoding_key, &validation)
        .map_err(|_| "id_token verification failed".to_string())?;

    if decoded.claims.iss != issuer {
        return Err("issuer mismatch".to_string());
    }

    if !aud_contains(&decoded.claims.aud, client_id) {
        return Err("audience mismatch".to_string());
    }

    if let Some(expected) = expected_nonce {
        if decoded.claims.nonce.as_deref() != Some(expected) {
            return Err("nonce mismatch".to_string());
        }
    }

    let claims = decoded.claims;
    let _ = claims.exp;
    let _ = claims.iat;
    Ok(claims)
}

fn aud_contains(aud: &serde_json::Value, client_id: &str) -> bool {
    match aud {
        serde_json::Value::String(s) => s == client_id,
        serde_json::Value::Array(arr) => arr.iter().any(|v| v.as_str() == Some(client_id)),
        _ => false,
    }
}

fn pkce_challenge(verifier: &str) -> String {
    let mut h = Sha256::new();
    h.update(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(h.finalize())
}

fn read_flow_cookie(jar: &CookieJar, name: &str) -> Result<FlowCookiePayload, ()> {
    let cookie = jar.get(name).ok_or(())?;
    let bytes = URL_SAFE_NO_PAD.decode(cookie.value()).map_err(|_| ())?;
    serde_json::from_slice(&bytes).map_err(|_| ())
}

fn build_flow_cookie(state: &AppState, name: &str, payload: &str, path: &str) -> Cookie<'static> {
    let mut cookie = Cookie::new(name.to_string(), payload.to_string());
    cookie.set_http_only(true);
    cookie.set_path(path.to_string());
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(!state.settings.debug);
    if let Some(domain) = state.settings.auth.cookie_domain.clone() {
        if !domain.is_empty() {
            cookie.set_domain(domain);
        }
    }
    cookie
}

fn clear_named_cookie(state: &AppState, name: &str, path: &str) -> Cookie<'static> {
    let mut cookie = Cookie::new(name.to_string(), "");
    cookie.set_http_only(true);
    cookie.set_path(path.to_string());
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(!state.settings.debug);
    cookie.make_removal();
    if let Some(domain) = state.settings.auth.cookie_domain.clone() {
        if !domain.is_empty() {
            cookie.set_domain(domain);
        }
    }
    cookie
}

fn build_sso_callback_url(state: &AppState, connection_id: Uuid) -> String {
    if let Some(base) = state
        .settings
        .auth
        .sso
        .base_url
        .as_ref()
        .filter(|s| !s.is_empty())
    {
        format!(
            "{}/api/auth/sso/{}/callback",
            base.trim_end_matches('/'),
            connection_id
        )
    } else {
        format!(
            "http://localhost:{}/api/auth/sso/{}/callback",
            state.settings.port, connection_id
        )
    }
}

fn frontend_post_auth_url(state: &AppState) -> String {
    state
        .settings
        .auth
        .frontend_origin
        .as_ref()
        .filter(|s| !s.is_empty())
        .map(|s| format!("{}/auth/callback", s.trim_end_matches('/')))
        .unwrap_or_else(|| "/auth/callback".to_string())
}

const REFRESH_COOKIE_NAME: &str = "sr_refresh";

fn hash_password(password: &str) -> Result<String, ()> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| ())?
        .to_string();
    Ok(hash)
}

fn verify_password(password: &str, password_hash: &str) -> Result<(), ()> {
    let parsed_hash = PasswordHash::new(password_hash).map_err(|_| ())?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| ())
}

fn sha256_hex(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    hex::encode(h.finalize())
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn build_refresh_cookie(state: &AppState, token: &str) -> Cookie<'static> {
    let mut cookie = Cookie::new(REFRESH_COOKIE_NAME, token.to_string());
    cookie.set_http_only(true);
    cookie.set_path("/api/auth");
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(!state.settings.debug);
    if let Some(domain) = state.settings.auth.cookie_domain.clone() {
        if !domain.is_empty() {
            cookie.set_domain(domain);
        }
    }
    cookie
}

fn clear_refresh_cookie(state: &AppState) -> Cookie<'static> {
    let mut cookie = Cookie::new(REFRESH_COOKIE_NAME, "");
    cookie.set_http_only(true);
    cookie.set_path("/api/auth");
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(!state.settings.debug);
    cookie.make_removal();
    if let Some(domain) = state.settings.auth.cookie_domain.clone() {
        if !domain.is_empty() {
            cookie.set_domain(domain);
        }
    }
    cookie
}

async fn issue_tokens_and_set_cookie(
    state: &Arc<AppState>,
    user_id: Uuid,
    role: String,
    rotated_from: Option<i32>,
) -> (CookieJar, Json<ApiResponse<LoginResponse>>) {
    let db = state.db.as_ref();
    let now = Utc::now();

    let signing_key = match state.settings.auth.jwt.signing_key.clone() {
        Some(k) => k,
        None => {
            return (
                CookieJar::new(),
                Json(ApiResponse::error(
                    500,
                    "jwt signing key not configured".to_string(),
                )),
            )
        }
    };

    let access_ttl = Duration::seconds(state.settings.auth.jwt.access_ttl_seconds);
    let refresh_ttl = Duration::seconds(state.settings.auth.jwt.refresh_ttl_seconds);

    let claims = JwtClaims {
        iss: state.settings.auth.jwt.issuer.clone(),
        aud: state.settings.auth.jwt.audience.clone(),
        sub: user_id.to_string(),
        role: role.clone(),
        iat: now.timestamp(),
        exp: (now + access_ttl).timestamp(),
    };

    let jwt = match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(signing_key.as_bytes()),
    ) {
        Ok(t) => t,
        Err(_) => {
            return (
                CookieJar::new(),
                Json(ApiResponse::error(500, "failed to sign jwt".to_string())),
            )
        }
    };

    let refresh = random_token();
    let refresh_hash = sha256_hex(&refresh);
    let expires_at = (now + refresh_ttl).naive_utc();

    let token_am = refresh_tokens::ActiveModel {
        user_id: Set(user_id),
        token_hash: Set(refresh_hash),
        rotated_from: Set(rotated_from),
        expires_at: Set(expires_at),
        revoked_at: Set(None),
        user_agent: Set(None),
        ip: Set(None),
        created_at: Set(now.naive_utc()),
        ..Default::default()
    };

    let _token_model = match token_am.insert(db).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("Failed to create refresh token: {}", e);
            return (
                CookieJar::new(),
                Json(ApiResponse::error(
                    500,
                    "failed to create refresh token".to_string(),
                )),
            );
        }
    };

    if let Some(old_id) = rotated_from {
        if let Ok(Some(old)) = RefreshTokens::find_by_id(old_id).one(db).await {
            let mut old_am: refresh_tokens::ActiveModel = old.into_active_model();
            old_am.revoked_at = Set(Some(now.naive_utc()));
            let _ = old_am.update(db).await;
        }
    }

    let cookie = build_refresh_cookie(state, &refresh);
    let jar = CookieJar::new().add(cookie);

    (
        jar,
        Json(ApiResponse::success(LoginResponse {
            access_token: jwt,
            token_type: "Bearer",
            expires_in: access_ttl.num_seconds(),
        })),
    )
}

fn origin_allowed_by_config(
    debug: bool,
    frontend_origin: Option<&str>,
    request_origin: Option<&str>,
) -> bool {
    if debug {
        return true;
    }

    let allowed_origins = parse_frontend_origins(frontend_origin);
    if allowed_origins.is_empty() {
        return true;
    }

    origin_matches(&allowed_origins, request_origin.unwrap_or(""))
}

fn origin_allowed(state: &AppState, headers: &HeaderMap) -> bool {
    let origin = headers.get(ORIGIN).and_then(|v| v.to_str().ok());
    origin_allowed_by_config(
        state.settings.debug,
        state.settings.auth.frontend_origin.as_deref(),
        origin,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use migration::MigratorTrait;
    use tower::ServiceExt;

    async fn setup_db() -> Result<
        (
            std::sync::Arc<sea_orm::DatabaseConnection>,
            std::sync::Arc<crate::AppState>,
        ),
        Box<dyn std::error::Error>,
    > {
        let db = sea_orm::Database::connect("sqlite::memory:").await?;
        migration::Migrator::up(&db, None).await?;

        let settings = test_settings("test-signing-key");
        let db_arc = std::sync::Arc::new(db);
        let (repos, services) = common::build_all(db_arc.clone(), &settings).await;

        let state = Arc::new(crate::AppState {
            db: db_arc.clone(),
            settings,
            repos,
            services,
        });

        Ok((db_arc, state))
    }

    fn test_settings(signing_key: &str) -> common::settings::Settings {
        let mut auth = common::settings::AuthSettings::default();
        auth.jwt.signing_key = Some(signing_key.to_string());
        common::settings::Settings {
            port: 3000,
            database: common::settings::DatabaseSettings {
                url: "sqlite::memory:".to_string(),
            },
            s3: common::settings::S3Settings {
                bucket: "test".to_string(),
                region: "us-east-1".to_string(),
                endpoint: None,
                access_key_id: None,
                secret_access_key: None,
                force_path_style: false,
            },
            github: common::settings::GithubSettings {
                search_keywords: "topic:agent-skill".to_string(),
                token: None,
                api_url: "https://api.github.com".to_string(),
            },
            worker: common::settings::WorkerSettings {
                scan_interval_seconds: 3600,
            },
            temporal: common::settings::TemporalSettings {
                server_url: "http://localhost:7233".to_string(),
                task_queue: "test".to_string(),
            },
            auth,
            debug: true,
        }
    }

    #[tokio::test]
    async fn local_register_then_me_works() {
        let (_db, state) = setup_db().await.unwrap();

        let app = axum::Router::new()
            .nest("/api/auth", router())
            .route("/api/me", axum::routing::get(me))
            .with_state(state);

        let req = Request::builder()
            .method("POST")
            .uri("/api/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"username":"alice","password":"password123","email":"a@example.com"}"#,
            ))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["code"], 200);
        let access = json["data"]["access_token"].as_str().unwrap().to_string();

        let req = Request::builder()
            .method("GET")
            .uri("/api/me")
            .header("authorization", format!("Bearer {}", access))
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["code"], 200);
        assert_eq!(json["data"]["username"], "alice");
    }

    #[test]
    fn origin_allowlist_accepts_multiple_origins() {
        let configured = Some("https://app.example.com,https://admin.example.com");
        assert!(origin_allowed_by_config(
            false,
            configured,
            Some("https://app.example.com")
        ));
        assert!(origin_allowed_by_config(
            false,
            configured,
            Some("https://admin.example.com")
        ));
    }

    #[test]
    fn origin_allowlist_rejects_unknown_origin() {
        let configured = Some("https://app.example.com,https://admin.example.com");
        assert!(!origin_allowed_by_config(
            false,
            configured,
            Some("https://evil.example.com")
        ));
    }
}
