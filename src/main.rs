mod auth;
mod crypto;
mod db;
mod storage;
mod upload;
mod routes;

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
    get_user_settings, update_user_settings, generate_outfit_preview, test_qwen_connection,
};

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::Pool<sqlx::Postgres>,
    pub storage: SupabaseStorage,
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

    let state = Arc::new(AppState {
        db,
        storage,
        jwt_secret,
        encryption_key,
    });

    let app = Router::new()
        .route("/tables", get(list_tables))
        .route("/db/:table", get(get_table_rows).post(insert_table_row))
        .route("/db/:table/:id", axum::routing::patch(update_table_row).delete(delete_table_row))
        .route("/settings", get(get_user_settings).put(update_user_settings))
        .route("/settings/test-qwen", get(test_qwen_connection))
        .route("/outfits", get(list_outfits).post(create_outfit))
        .route("/outfits/:id", axum::routing::delete(delete_outfit))
        .route("/outfits/generate-preview", post(generate_outfit_preview))
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
