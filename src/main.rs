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
use routes::{list_users, upload_file};

#[derive(Clone)]
pub struct AppState {
    db: sqlx::Pool<sqlx::Postgres>,
    storage: SupabaseStorage,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let db = connect_db().await;
    let storage = SupabaseStorage::new();

    let state = Arc::new(AppState { db, storage });

    let app = Router::new()
        .route("/users", get(list_users))
        .route("/upload", post(upload_file))
        .with_state(state);

    println!("🚀 running on http://localhost:3000");
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("🚀 running on http://localhost:3000");
    
    serve(listener, app).await.unwrap();
}
