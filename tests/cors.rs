//! CORS middleware tests without a database: the same `cors` middleware
//! `build_router` layers (outermost) is layered over a trivial handler, so
//! these run on any machine even when no Postgres is available.

use std::convert::Infallible;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware;
use axum::response::Response;
use backend::cors::{cors, CorsAllowlist};
use tower::util::ServiceExt;
use tower::{service_fn, Layer, Service};

const ALLOWED: &str = "https://cloth.example";

/// The CORS middleware over a handler that answers `secret` whenever it is
/// reached — so an empty body proves the preflight short-circuited.
fn app(
    allow: CorsAllowlist,
) -> impl Service<Request<Body>, Response = Response, Error = Infallible> + Clone {
    let handler = service_fn(move |_req: Request<Body>| {
        Box::pin(async { Ok::<Response, Infallible>(Response::new(Body::from("secret"))) })
    });
    middleware::from_fn_with_state(allow, cors).layer(handler)
}

fn req(method: &str, origin: Option<&str>, preflight: bool) -> Request<Body> {
    let mut b = Request::builder().method(method).uri("/anything");
    if let Some(o) = origin {
        b = b.header("origin", o);
    }
    if preflight {
        b = b.header("access-control-request-method", "GET");
    }
    b.body(Body::empty()).unwrap()
}

async fn call(
    app: &(impl Service<Request<Body>, Response = Response, Error = Infallible> + Clone),
    r: Request<Body>,
) -> Response {
    app.clone().oneshot(r).await.unwrap()
}

async fn text(resp: Response) -> String {
    String::from_utf8(
        axum::body::to_bytes(resp.into_body(), 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

#[tokio::test]
async fn preflight_allowed_origin_is_answered_204_without_reaching_the_handler() {
    let app = app(CorsAllowlist(vec![ALLOWED.to_string()]));
    let resp = call(&app, req("OPTIONS", Some(ALLOWED), true)).await;

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        resp.headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some(ALLOWED)
    );
    let methods = resp
        .headers()
        .get("access-control-allow-methods")
        .and_then(|v| v.to_str().ok())
        .unwrap();
    for m in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
        assert!(
            methods.split(',').any(|x| x.trim() == m),
            "{m} missing from {methods}"
        );
    }
    let headers = resp
        .headers()
        .get("access-control-allow-headers")
        .and_then(|v| v.to_str().ok())
        .unwrap();
    assert!(headers.contains("Authorization") && headers.contains("Content-Type"));
    assert!(resp.headers().get("access-control-max-age").is_some());
    // The handler never ran: empty body, not the handler's "secret".
    assert_eq!(text(resp).await, "");
}

#[tokio::test]
async fn preflight_disallowed_origin_gets_no_allow_origin_header() {
    let app = app(CorsAllowlist(vec![ALLOWED.to_string()]));
    let resp = call(&app, req("OPTIONS", Some("https://evil.example"), true)).await;

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert!(resp.headers().get("access-control-allow-origin").is_none());
}

#[tokio::test]
async fn preflight_without_origin_gets_no_allow_origin_header() {
    let app = app(CorsAllowlist(vec![ALLOWED.to_string()]));
    let resp = call(&app, req("OPTIONS", None, true)).await;

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert!(resp.headers().get("access-control-allow-origin").is_none());
}

#[tokio::test]
async fn simple_request_from_allowed_origin_carries_allow_origin_and_expose_headers() {
    let app = app(CorsAllowlist(vec![ALLOWED.to_string()]));
    let resp = call(&app, req("GET", Some(ALLOWED), false)).await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some(ALLOWED)
    );
    // The SPA reads X-Total-Count cross-origin; it must be exposed.
    let expose = resp
        .headers()
        .get("access-control-expose-headers")
        .and_then(|v| v.to_str().ok())
        .unwrap();
    assert!(expose.contains("X-Total-Count"));
    assert_eq!(
        resp.headers().get("vary").and_then(|v| v.to_str().ok()),
        Some("Origin")
    );
    // The handler ran.
    assert_eq!(text(resp).await, "secret");
}

#[tokio::test]
async fn simple_request_from_disallowed_origin_runs_but_is_unreadable_by_the_browser() {
    let app = app(CorsAllowlist(vec![ALLOWED.to_string()]));
    let resp = call(&app, req("GET", Some("https://evil.example"), false)).await;

    // The server cannot (and should not) block the request itself; the
    // browser blocks the read because no allow-origin header is set.
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().get("access-control-allow-origin").is_none());
    // Caches must key on Origin: the headers differ by origin.
    assert_eq!(
        resp.headers().get("vary").and_then(|v| v.to_str().ok()),
        Some("Origin")
    );
    assert_eq!(text(resp).await, "secret");
}

#[tokio::test]
async fn same_origin_request_without_origin_header_gets_no_cors_headers() {
    let app = app(CorsAllowlist(vec![ALLOWED.to_string()]));
    let resp = call(&app, req("GET", None, false)).await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().get("access-control-allow-origin").is_none());
    assert!(resp
        .headers()
        .get("access-control-expose-headers")
        .is_none());
}

#[tokio::test]
async fn empty_allowlist_emits_no_cors_headers_at_all() {
    let app = app(CorsAllowlist::default());
    let resp = call(&app, req("OPTIONS", Some(ALLOWED), true)).await;
    assert!(resp.headers().get("access-control-allow-origin").is_none());
    let resp = call(&app, req("GET", Some(ALLOWED), false)).await;
    assert!(resp.headers().get("access-control-allow-origin").is_none());
}
