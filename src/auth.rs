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

/// Verify a Supabase access token against the project's JWT secret.
///
/// The token must be HS256, carry audience `authenticated`, role
/// `authenticated`, and an `iss` equal to the Supabase project URL (`issuer`).
/// Validating the issuer means a token minted by a different project that
/// happens to share the secret cannot authenticate here.
pub fn verify_token(token: &str, secret: &str, issuer: &str) -> Result<Uuid, String> {
    let decoding_key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());

    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_audience(&["authenticated"]);
    // `iss` must be present and equal to the Supabase project URL.
    validation.set_issuer(&[issuer]);
    validation.required_spec_claims = std::collections::HashSet::from(["iss".to_string()]);

    let data = jsonwebtoken::decode::<Claims>(token, &decoding_key, &validation)
        .map_err(|e| e.to_string())?;

    // only accept regular user tokens, not service_role
    if data.claims.role.as_deref() != Some("authenticated") {
        return Err("invalid token role".to_string());
    }

    Uuid::parse_str(&data.claims.sub).map_err(|e| e.to_string())
}

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
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, "missing bearer token".to_string()))?;

        let user_id = verify_token(token, &state.jwt_secret, &state.jwt_issuer)
            .map_err(|e| (StatusCode::UNAUTHORIZED, e))?;

        Ok(AuthUser(user_id))
    }
}

/// How long a signed image-serving token stays valid.
pub const FILE_TOKEN_TTL_SECS: i64 = 600;

pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn file_token_mac_inputs(mac: &mut hmac::Hmac<sha2::Sha256>, user_id: &Uuid, exp: i64) {
    use hmac::Mac;
    mac.update(b"cloth-web:file-token:");
    mac.update(user_id.as_bytes());
    mac.update(&exp.to_be_bytes());
}

/// Sign a short-lived token that authorizes serving images from `user_id`'s
/// own storage folder until `exp` (unix seconds). Base64url, no padding.
///
/// This replaces putting the JWT in image URLs (which leaked into access
/// logs): the tag is a 32-byte HMAC bound to the user and its expiry, valid
/// only for `?token=...&exp=...` on `/files` paths inside that user's folder.
pub fn sign_file_token(key: &[u8; 32], user_id: &Uuid, exp: i64) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
    use base64::Engine;
    use hmac::{Hmac, Mac};

    let mut mac = <Hmac<sha2::Sha256>>::new_from_slice(key).expect("HMAC accepts any key length");
    file_token_mac_inputs(&mut mac, user_id, exp);
    B64.encode(mac.finalize().into_bytes())
}

/// Verify a signed image token, comparing constant-time (`Mac::verify_slice`).
pub fn verify_file_token(key: &[u8; 32], user_id: &Uuid, exp: i64, token: &str) -> bool {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
    use base64::Engine;
    use hmac::{Hmac, Mac};

    let bytes = match B64.decode(token.as_bytes()) {
        Ok(b) if b.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            arr
        }
        _ => return false,
    };

    let mut mac = <Hmac<sha2::Sha256>>::new_from_slice(key).expect("HMAC accepts any key length");
    file_token_mac_inputs(&mut mac, user_id, exp);
    mac.verify_slice(&bytes).is_ok()
}

/// Authentication for `/files/*path`. `<img>` tags cannot set headers, so this
/// accepts the standard Bearer JWT **or** a short-lived signed token from
/// `GET /files/sign` in the query. The JWT is never placed in a URL by the
/// client, so access logs no longer carry full-access credentials.
pub struct FileAuth(pub Uuid);

#[async_trait]
impl FromRequestParts<Arc<AppState>> for FileAuth {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        // Preferred: a regular Bearer JWT.
        if let Some(bearer) = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
        {
            return verify_token(bearer, &state.jwt_secret, &state.jwt_issuer)
                .map_err(|e| (StatusCode::UNAUTHORIZED, e))
                .map(FileAuth);
        }

        // Signed image token: `?token=<base64url-hmac>&exp=<unix>`. The tag
        // binds the user id parsed from the path, so the token can only serve
        // the signer's own folder.
        let mut params = std::collections::HashMap::new();
        for pair in parts.uri.query().unwrap_or("").split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                params.insert(k.to_string(), v.to_string());
            }
        }
        let sig = params.get("token").map(|s| s.as_str()).unwrap_or("");
        let exp = params
            .get("exp")
            .and_then(|s| s.parse::<i64>().ok())
            .filter(|e| *e > unix_now())
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    "missing or expired file token".into(),
                )
            })?;

        let user_id = parts
            .uri
            .path()
            .split('/')
            .nth(3) // /files/users/{user_id}/...
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, "malformed file path".into()))?;

        if !verify_file_token(&state.encryption_key, &user_id, exp, sig) {
            return Err((StatusCode::UNAUTHORIZED, "invalid file token".into()));
        }
        Ok(FileAuth(user_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "test-jwt-secret";
    const ISSUER: &str = "https://example.supabase.co";
    const USER: &str = "11111111-1111-4111-8111-111111111111";

    fn make_token(secret: &str, issuer: &str, role: &str, expires_in_secs: i64) -> String {
        let exp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + expires_in_secs;
        let claims = serde_json::json!({
            "sub": USER,
            "aud": "authenticated",
            "role": role,
            "iss": issuer,
            "exp": exp,
        });
        jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    #[test]
    fn accepts_valid_user_token() {
        let token = make_token(SECRET, ISSUER, "authenticated", 3600);
        assert_eq!(
            verify_token(&token, SECRET, ISSUER).unwrap(),
            Uuid::parse_str(USER).unwrap()
        );
    }

    #[test]
    fn rejects_wrong_issuer() {
        let token = make_token(SECRET, "https://other.supabase.co", "authenticated", 3600);
        assert!(verify_token(&token, SECRET, ISSUER).is_err());
    }

    #[test]
    fn rejects_service_role() {
        let token = make_token(SECRET, ISSUER, "service_role", 3600);
        assert!(verify_token(&token, SECRET, ISSUER).is_err());
    }

    #[test]
    fn rejects_expired_token() {
        let token = make_token(SECRET, ISSUER, "authenticated", -3600);
        assert!(verify_token(&token, SECRET, ISSUER).is_err());
    }

    #[test]
    fn rejects_wrong_secret() {
        let token = make_token("a-different-secret", ISSUER, "authenticated", 3600);
        assert!(verify_token(&token, SECRET, ISSUER).is_err());
    }

    #[test]
    fn file_token_roundtrip() {
        let key = [7u8; 32];
        let user = Uuid::parse_str(USER).unwrap();
        let exp = 1_999_999_999;

        let token = sign_file_token(&key, &user, exp);
        assert!(verify_file_token(&key, &user, exp, &token));
    }

    #[test]
    fn file_token_rejects_other_user() {
        let key = [7u8; 32];
        let user = Uuid::parse_str(USER).unwrap();
        let other = Uuid::new_v4();
        let exp = 1_999_999_999;

        let token = sign_file_token(&key, &user, exp);
        assert!(!verify_file_token(&key, &other, exp, &token));
    }

    #[test]
    fn file_token_rejects_other_expiry_and_key() {
        let key = [7u8; 32];
        let other_key = [8u8; 32];
        let user = Uuid::parse_str(USER).unwrap();
        let exp = 1_999_999_999;

        let token = sign_file_token(&key, &user, exp);
        assert!(!verify_file_token(&key, &user, exp + 1, &token));
        assert!(!verify_file_token(&other_key, &user, exp, &token));
    }

    #[test]
    fn file_token_rejects_garbage() {
        let key = [7u8; 32];
        let user = Uuid::parse_str(USER).unwrap();
        assert!(!verify_file_token(&key, &user, 1, ""));
        assert!(!verify_file_token(&key, &user, 1, "not-base64!!!"));
        assert!(!verify_file_token(&key, &user, 1, "AAAABBBB"));
    }
}
