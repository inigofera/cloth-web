use std::sync::Arc;

use axum::serve;
use backend::{build_router, db, storage::SupabaseStorage, AppState};
use dotenv::dotenv;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let db = db::connect_db().await;
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

    // Startup sweep: import any Discord outfits posted since the last run, for
    // the configured user (or the largest active wardrobe).
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
            match backend::discord::sync_discord_for_user(&db_for_sweep, user_id).await {
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
        image_cache: Arc::new(backend::cache::ImageCache::new(512, 24 * 1024 * 1024)),
        rate: Arc::new(backend::ratelimit::RateLimiter::new()),
        schema,
        jwt_secret,
        jwt_issuer,
        encryption_key,
    });

    let app = build_router(state);

    tracing::info!("running on http://localhost:3000");
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();

    serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server failed");
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
