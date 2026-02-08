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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::dto::JwtClaims;
    use axum::body::Body;
    use axum::http::Request;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use migration::MigratorTrait;

    async fn setup_state(signing_key: Option<&str>) -> Arc<AppState> {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        migration::Migrator::up(&db, None).await.unwrap();

        let mut settings = common::settings::Settings::default();
        settings.database.url = "sqlite::memory:".to_string();
        settings.auth.jwt.signing_key = signing_key.map(ToString::to_string);

        let db_arc = Arc::new(db);
        let (repos, services) = common::build_all(db_arc.clone(), &settings).await.unwrap();

        Arc::new(AppState {
            db: db_arc,
            settings,
            services,
            repos,
            allowed_frontend_origins: Arc::new(vec![]),
        })
    }

    fn make_token(state: &AppState, sub: &str) -> String {
        let claims = JwtClaims {
            iss: state.settings.auth.jwt.issuer.clone(),
            aud: state.settings.auth.jwt.audience.clone(),
            sub: sub.to_string(),
            role: "user".to_string(),
            iat: chrono::Utc::now().timestamp(),
            exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(
                state
                    .settings
                    .auth
                    .jwt
                    .signing_key
                    .as_ref()
                    .unwrap()
                    .as_bytes(),
            ),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn from_request_parts_rejects_missing_bearer() {
        let state = setup_state(Some("test-key")).await;
        let req = Request::builder().body(Body::empty()).unwrap();
        let (mut parts, _) = req.into_parts();

        let err = AuthUser::from_request_parts(&mut parts, &state)
            .await
            .unwrap_err();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
        assert_eq!(err.1 .0.code, 401);
    }

    #[tokio::test]
    async fn from_request_parts_requires_signing_key() {
        let state = setup_state(None).await;
        let req = Request::builder()
            .header("authorization", "Bearer token")
            .body(Body::empty())
            .unwrap();
        let (mut parts, _) = req.into_parts();

        let err = AuthUser::from_request_parts(&mut parts, &state)
            .await
            .unwrap_err();
        assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(err.1 .0.code, 500);
    }

    #[tokio::test]
    async fn from_request_parts_accepts_valid_token_and_rejects_invalid_sub() {
        let state = setup_state(Some("test-key")).await;
        let valid = make_token(&state, &uuid::Uuid::new_v4().to_string());
        let req = Request::builder()
            .header("authorization", format!("Bearer {}", valid))
            .body(Body::empty())
            .unwrap();
        let (mut parts, _) = req.into_parts();
        let user = match AuthUser::from_request_parts(&mut parts, &state).await {
            Ok(user) => user,
            Err(_) => panic!("valid token should be accepted"),
        };
        assert_eq!(user.role, "user");

        let invalid_sub = make_token(&state, "not-a-uuid");
        let req = Request::builder()
            .header("authorization", format!("Bearer {}", invalid_sub))
            .body(Body::empty())
            .unwrap();
        let (mut parts, _) = req.into_parts();
        let err = AuthUser::from_request_parts(&mut parts, &state)
            .await
            .unwrap_err();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
        assert_eq!(err.1 .0.code, 401);
    }
}
