pub mod auth;
pub mod cache;
pub mod crypto;
pub mod db;
pub mod discord;
pub mod ratelimit;
pub mod routes;
pub mod storage;
pub mod upload;

use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;

use routes::{
    create_category, create_color, create_outfit, create_subcategory, delete_outfit,
    delete_table_row, generate_discord_link_code, generate_outfit_preview, get_table_rows,
    get_user_settings, insert_table_row, list_outfits, list_tables, serve_file,
    sign_file_token_request, sync_discord_outfits, test_qwen_connection, unlink_discord_account,
    update_table_row, update_user_settings, upload_file,
};

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::Pool<sqlx::Postgres>,
    pub storage: storage::SupabaseStorage,
    pub image_cache: Arc<cache::ImageCache>,
    pub rate: Arc<ratelimit::RateLimiter>,
    // The shape of the public schema, read once at startup: the CRUD endpoints answer their
    // table questions from here rather than from information_schema per request.
    pub schema: Arc<db::SchemaCache>,
    pub jwt_secret: String,
    // Supabase project URL: the `iss` claim every valid access token carries.
    pub jwt_issuer: String,
    pub encryption_key: [u8; 32],
    // Gemini REST base (`.../v1beta`). Overridable so the integration tests can
    // point try-on at an in-process mock; production keeps the real endpoint.
    pub gemini_api_base: String,
}

/// Hygiene headers on every response. The SPA itself is served by the tunnel
/// or a static host, so its own headers (CSP, `no-cache` on the shell) are
/// configured there; these cover everything the API answers.
async fn security_headers(req: Request<axum::body::Body>, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let headers = resp.headers_mut();
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("referrer-policy", "no-referrer".parse().unwrap());
    resp
}

/// One line per request at `debug` level (RUST_LOG), so a quiet server still
/// leaves an audit trail when someone actually wants to look.
async fn request_log(req: Request<axum::body::Body>, next: Next) -> Response {
    let start = std::time::Instant::now();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let resp = next.run(req).await;
    tracing::debug!(
        method = %method,
        path = %path,
        status = resp.status().as_u16(),
        elapsed_ms = start.elapsed().as_millis(),
        "request"
    );
    resp
}

/// The full API router, state-wired. `main()` and the integration tests build
/// the same app this way, so what the tests exercise is exactly what ships.
pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Unauthenticated on purpose: the tunnel and any uptime monitor hit it.
        .route("/healthz", get(routes::healthz))
        .route("/tables", get(list_tables))
        .route("/db/:table", get(get_table_rows).post(insert_table_row))
        .route(
            "/db/:table/:id",
            axum::routing::patch(update_table_row).delete(delete_table_row),
        )
        .route(
            "/settings",
            get(get_user_settings).put(update_user_settings),
        )
        .route("/settings/test-qwen", get(test_qwen_connection))
        .route("/settings/discord-code", post(generate_discord_link_code))
        .route("/settings/discord-unlink", post(unlink_discord_account))
        .route("/outfits", get(list_outfits).post(create_outfit))
        .route("/outfits/:id", axum::routing::delete(delete_outfit))
        .route("/outfits/generate-preview", post(generate_outfit_preview))
        .route("/outfits/discord-sync", post(sync_discord_outfits))
        .route("/colors", post(create_color))
        .route("/clothing-categories", post(create_category))
        .route("/clothing-subcategories", post(create_subcategory))
        .route("/upload", post(upload_file))
        // Static segments win over the `/files/*path` wildcard below.
        .route("/files/sign", get(sign_file_token_request))
        .route("/files/*path", get(serve_file))
        .layer(axum::extract::DefaultBodyLimit::max(25 * 1024 * 1024))
        .layer(axum::middleware::from_fn(security_headers))
        .layer(axum::middleware::from_fn(request_log))
        .with_state(state)
}
