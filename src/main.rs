mod auth;
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
use routes::{upload_file, serve_file, list_tables, get_table_rows, insert_table_row, delete_table_row, update_table_row, list_outfits, create_outfit, delete_outfit, create_color, create_category, create_subcategory};

#[derive(Clone)]
pub struct AppState {
    db: sqlx::Pool<sqlx::Postgres>,
    storage: SupabaseStorage,
    jwt_secret: String,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let db = connect_db().await;
    let storage = SupabaseStorage::new();

    let jwt_secret = std::env::var("SUPABASE_JWT_SECRET").expect("SUPABASE_JWT_SECRET not set");

    let state = Arc::new(AppState {
        db,
        storage,
        jwt_secret,
    });

    let app = Router::new()
        .route("/tables", get(list_tables))
        .route("/db/:table", get(get_table_rows).post(insert_table_row))
        .route("/db/:table/:id", axum::routing::delete(delete_table_row))
        .route("/db/:table/:id", axum::routing::patch(update_table_row))
        .route("/outfits", get(list_outfits).post(create_outfit))
        .route("/outfits/:id", axum::routing::delete(delete_outfit))
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
