mod auth;
mod cache;
mod crypto;
mod db;
mod discord;
mod ratelimit;
mod routes;
mod storage;
mod upload;

use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use axum::serve;
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use db::connect_db;
use dotenv::dotenv;
use routes::{
    create_category, create_color, create_outfit, create_subcategory, delete_outfit,
    delete_table_row, generate_discord_link_code, generate_outfit_preview, get_table_rows,
    get_user_settings, insert_table_row, list_outfits, list_tables, serve_file,
    sign_file_token_request, sync_discord_outfits, test_qwen_connection, unlink_discord_account,
    update_table_row, update_user_settings, upload_file,
};
use std::sync::Arc;
use storage::SupabaseStorage;
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::Pool<sqlx::Postgres>,
    pub storage: SupabaseStorage,
    pub image_cache: Arc<cache::ImageCache>,
    pub rate: Arc<ratelimit::RateLimiter>,
    // The shape of the public schema, read once at startup: the CRUD endpoints answer their
    // table questions from here rather than from information_schema per request.
    pub schema: Arc<db::SchemaCache>,
    pub jwt_secret: String,
    // Supabase project URL: the `iss` claim every valid access token carries.
    pub jwt_issuer: String,
    pub encryption_key: [u8; 32],
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

/// Wait for Ctrl+C or SIGTERM (docker stop), then let axum drain in-flight
/// requests instead of dying mid-write.
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to register SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }
    }
    #[cfg(not(unix))]
    let _ = ctrl_c.await;
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let db = connect_db().await;
    let storage = SupabaseStorage::new();

    let jwt_secret = std::env::var("SUPABASE_JWT_SECRET").expect("SUPABASE_JWT_SECRET not set");
    let jwt_issuer = std::env::var("SUPABASE_URL").expect("SUPABASE_URL not set");

    let encryption_key_hex = std::env::var("SETTINGS_ENCRYPTION_KEY")
        .expect("SETTINGS_ENCRYPTION_KEY must be set in environment");
    let key_bytes = hex::decode(encryption_key_hex.trim())
        .expect("SETTINGS_ENCRYPTION_KEY must be a valid 64-character hex string");
    if key_bytes.len() != 32 {
        panic!(
            "SETTINGS_ENCRYPTION_KEY must be exactly 32 bytes (64 hex characters), got {} bytes",
            key_bytes.len()
        );
    }
    let mut encryption_key = [0u8; 32];
    encryption_key.copy_from_slice(&key_bytes);

    let db_for_sweep = db.clone();
    tokio::spawn(async move {
        let user_id_res = if let Ok(uid_str) = std::env::var("DISCORD_DEFAULT_USER_ID") {
            uuid::Uuid::parse_str(&uid_str).ok()
        } else {
            sqlx::query_scalar::<_, uuid::Uuid>(
                r#"SELECT user_id FROM "clothing-items" WHERE is_active = true GROUP BY user_id ORDER BY count(*) DESC LIMIT 1"#
            )
            .fetch_optional(&db_for_sweep)
            .await
            .unwrap_or(None)
        };

        if let Some(user_id) = user_id_res {
            tracing::info!(%user_id, "discord sweep: checking for new outfits");
            match discord::sync_discord_for_user(&db_for_sweep, user_id).await {
                Ok(res) => {
                    if res.imported_count > 0 {
                        tracing::info!(
                            imported = res.imported_count,
                            "discord sweep: imported outfits on launch"
                        );
                        for s in &res.summary {
                            tracing::info!(line = %s, "discord sweep summary");
                        }
                    } else {
                        tracing::info!("discord sweep: up to date (no new outfits)");
                    }
                }
                Err(e) => {
                    tracing::error!(%e, "discord sweep failed at startup");
                }
            }
        }
    });

    // Warm the schema snapshot before the first request can arrive: one statement reads the
    // shape of every public table, and the CRUD endpoints never pay that read themselves.
    let schema = Arc::new(db::SchemaCache::new());
    schema.refresh(&db).await;

    let state = Arc::new(AppState {
        db,
        storage,
        image_cache: Arc::new(cache::ImageCache::new(512, 24 * 1024 * 1024)),
        rate: Arc::new(ratelimit::RateLimiter::new()),
        schema,
        jwt_secret,
        jwt_issuer,
        encryption_key,
    });

    let app = Router::new()
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
        .layer(DefaultBodyLimit::max(25 * 1024 * 1024))
        .layer(axum::middleware::from_fn(security_headers))
        .layer(axum::middleware::from_fn(request_log))
        .with_state(state);

    tracing::info!("running on http://localhost:3000");
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();

    serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server failed");
}
