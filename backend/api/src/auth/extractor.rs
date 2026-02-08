use super::dto::JwtClaims;
use crate::models::ApiResponse;
use crate::AppState;
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
    Json,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use std::sync::Arc;
use uuid::Uuid;

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
