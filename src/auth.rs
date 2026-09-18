use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub role: Option<String>,
}

pub struct AuthUser(pub Uuid);

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .or_else(|| token_from_query(parts))
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, "missing bearer token".to_string()))?;

        let decoding_key =
            jsonwebtoken::DecodingKey::from_secret(state.jwt_secret.as_bytes());

        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.set_audience(&["authenticated"]);

        let data = jsonwebtoken::decode::<Claims>(token, &decoding_key, &validation)
            .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

        // only accept regular user tokens, not service_role
        if data.claims.role.as_deref() != Some("authenticated") {
            return Err((StatusCode::UNAUTHORIZED, "invalid token role".to_string()));
        }

        let user_id = Uuid::parse_str(&data.claims.sub)
            .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

        Ok(AuthUser(user_id))
    }
}

fn token_from_query(parts: &Parts) -> Option<&str> {
    let query = parts.uri.query()?;
    query.split('&').find_map(|pair| {
        let mut kv = pair.splitn(2, '=');
        let key = kv.next()?;
        let value = kv.next()?;
        (key == "token").then_some(value)
    })
}
