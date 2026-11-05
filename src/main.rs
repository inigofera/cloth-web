mod discord;
mod auth;
mod crypto;
mod db;
mod storage;
mod upload;
mod routes;
mod cache;

use axum::{Router, routing::{get, post}, extract::DefaultBodyLimit};
use axum::serve;
use tokio::net::TcpListener;
use std::sync::Arc;
use dotenv::dotenv;
use db::connect_db;
use storage::SupabaseStorage;
use routes::{
    upload_file, serve_file, list_tables, get_table_rows, insert_table_row,
    delete_table_row, update_table_row, list_outfits, create_outfit, delete_outfit,
    create_color, create_category, create_subcategory,
    get_user_settings, update_user_settings, generate_outfit_preview, test_qwen_connection, sync_discord_outfits, generate_discord_link_code, unlink_discord_account,
};

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::Pool<sqlx::Postgres>,
    pub storage: SupabaseStorage,
    pub image_cache: Arc<cache::ImageCache>,
    // The shape of the public schema, read once at startup: the CRUD endpoints answer their
    // table questions from here rather than from information_schema per request.
    pub schema: Arc<db::SchemaCache>,
    pub jwt_secret: String,
    pub encryption_key: [u8; 32],
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let db = connect_db().await;
    let storage = SupabaseStorage::new();

    let jwt_secret = std::env::var("SUPABASE_JWT_SECRET").expect("SUPABASE_JWT_SECRET not set");

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
            println!("?? [Discord Sweep] Checking for new outfits from Discord for user {}...", user_id);
            match discord::sync_discord_for_user(&db_for_sweep, user_id).await {
                Ok(res) => {
                    if res.imported_count > 0 {
                        println!("? [Discord Sweep] Successfully imported {} outfit(s) on launch!", res.imported_count);
                        for s in res.summary {
                            println!("   � {}", s);
                        }
                    } else {
                        println!("?? [Discord Sweep] Up to date (no new outfits found).");
                    }
                }
                Err(e) => {
                    eprintln!("?? [Discord Sweep] Error during startup sweep: {}", e);
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
        schema: schema,
        jwt_secret,
        encryption_key,
    });

    let app = Router::new()
        .route("/tables", get(list_tables))
        .route("/db/:table", get(get_table_rows).post(insert_table_row))
        .route("/db/:table/:id", axum::routing::patch(update_table_row).delete(delete_table_row))
        .route("/settings", get(get_user_settings).put(update_user_settings))
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
        .route("/files/*path", get(serve_file))
        .layer(DefaultBodyLimit::max(25 * 1024 * 1024))
        .with_state(state);

    println!("🚀 running on http://localhost:3000");
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();

    serve(listener, app).await.unwrap();
}
