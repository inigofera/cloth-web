mod auth;
mod db;
mod storage;
mod routes;

use axum::{Router, routing::{get, post}};
use axum::serve;
use tokio::net::TcpListener;
use std::sync::Arc;
use dotenv::dotenv;
use db::connect_db;
use storage::SupabaseStorage;
use routes::{upload_file, list_tables, get_table_rows, insert_table_row, delete_table_row, update_table_row};

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
        .route("/upload", post(upload_file))
        .with_state(state);

    println!("🚀 running on http://localhost:3000");
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();

    serve(listener, app).await.unwrap();
}
