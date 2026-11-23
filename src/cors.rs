//! CORS for the cross-origin deployment.
//!
//! The frontend is served from `cloth.inigoartolozaga.com` (GitHub Pages) and
//! the API from `api.inigoartolozaga.com` (Cloudflare tunnel → inibaby), so
//! every browser request is cross-origin. The allowlist is a set of exact
//! origins, read from `CORS_ALLOWED_ORIGINS` (comma-separated) at startup —
//! never `*`: the API carries `Authorization` headers, where `*` is invalid,
//! so a disallowed origin simply gets no `Access-Control-Allow-Origin`
//! header and the browser refuses to read the response.
//!
//! Preflight (`OPTIONS` + `Access-Control-Request-Method`) is answered here
//! with a bare 204 before the request can reach the JWT extractors: an
//! OPTIONS probe must never require auth.

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderValue, Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

/// The exact origins the browser is allowed to read responses from.
/// Empty (the default) means same-origin only: no CORS headers are ever
/// emitted, which is correct for the legacy single-origin deployment.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CorsAllowlist(pub Vec<String>);

impl CorsAllowlist {
    /// Parse the `CORS_ALLOWED_ORIGINS` environment variable
    /// (comma-separated exact origins, whitespace tolerated).
    pub fn from_env() -> Self {
        std::env::var("CORS_ALLOWED_ORIGINS")
            .ok()
            .map(|raw| Self(parse_origins(&raw)))
            .unwrap_or_default()
    }

    pub fn is_allowed(&self, origin: &str) -> bool {
        // Exact match: origins are normalized by the browser (lowercase
        // host, no trailing slash), so the env values should be too.
        self.0.iter().any(|o| o == origin)
    }
}

fn parse_origins(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// The methods the SPA actually uses. Also advertised on preflight.
const ALLOWED_METHODS: &str = "GET, POST, PUT, PATCH, DELETE, OPTIONS";
/// The request headers the SPA sends cross-origin.
const ALLOWED_HEADERS: &str = "Authorization, Content-Type";
/// Response headers the SPA reads cross-origin (JS can only see headers
/// named here): `X-Total-Count` drives the capped-list UI.
const EXPOSED_HEADERS: &str = "X-Total-Count";
/// Browsers cache preflight answers for this long (1 hour).
const MAX_AGE: &str = "3600";

// Note: `allow` must come before `req` — in axum 0.7 the last extractor
// before `Next` must implement the body-consuming `FromRequest`, which
// `State` does not.
pub async fn cors(allow: State<CorsAllowlist>, req: Request<Body>, next: Next) -> Response {
    // Owned copy: `req` is moved into the inner service below.
    let origin = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    // Preflight: answer without touching the router. A disallowed or missing
    // origin gets a 204 with no allow-origin header, which fails the
    // preflight in the browser.
    if req.method() == Method::OPTIONS
        && req
            .headers()
            .contains_key(header::ACCESS_CONTROL_REQUEST_METHOD)
    {
        let mut resp = Response::new(Body::empty());
        *resp.status_mut() = StatusCode::NO_CONTENT;
        if let Some(o) = allowed_origin(origin.as_deref(), &allow) {
            let h = resp.headers_mut();
            h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, o);
            h.insert(header::VARY, HeaderValue::from_static("Origin"));
            h.insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static(ALLOWED_METHODS),
            );
            h.insert(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                HeaderValue::from_static(ALLOWED_HEADERS),
            );
            h.insert(
                header::ACCESS_CONTROL_MAX_AGE,
                HeaderValue::from_static(MAX_AGE),
            );
        }
        return resp;
    }

    let mut resp = next.run(req).await;
    if let Some(o) = allowed_origin(origin.as_deref(), &allow) {
        let h = resp.headers_mut();
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, o);
        h.insert(header::VARY, HeaderValue::from_static("Origin"));
        h.insert(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            HeaderValue::from_static(EXPOSED_HEADERS),
        );
    } else if origin.is_some() {
        // The response differs by origin (header present or not), so caches
        // must key on it even when the origin is disallowed.
        resp.headers_mut()
            .insert(header::VARY, HeaderValue::from_static("Origin"));
    }
    resp
}

/// The request's origin, echoed as a header value, only when it is on the
/// allowlist.
fn allowed_origin(origin: Option<&str>, allow: &CorsAllowlist) -> Option<HeaderValue> {
    origin
        .filter(|o| allow.is_allowed(o))
        .and_then(|o| HeaderValue::from_str(o).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comma_separated_origins_with_whitespace() {
        let list = parse_origins("https://a.example, https://b.example , ,");
        assert_eq!(list, vec!["https://a.example", "https://b.example"]);
    }

    #[test]
    fn empty_or_blank_env_yields_empty_allowlist() {
        assert_eq!(parse_origins(""), Vec::<String>::new());
        assert_eq!(parse_origins("  ,  "), Vec::<String>::new());
    }

    #[test]
    fn allows_only_exact_origins() {
        let list = CorsAllowlist(vec!["https://cloth.example".to_string()]);
        assert!(list.is_allowed("https://cloth.example"));
        assert!(!list.is_allowed("http://cloth.example"));
        assert!(!list.is_allowed("https://cloth.example.evil.example"));
        assert!(!list.is_allowed("https://cloth.example/"));
        assert!(!list.is_allowed("HTTPS://cloth.example"));
    }

    #[test]
    fn empty_allowlist_allows_nothing() {
        assert!(!CorsAllowlist::default().is_allowed("https://cloth.example"));
    }
}
