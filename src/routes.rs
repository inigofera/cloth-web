use axum::{Json, extract::{Multipart, State}};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;
use sqlx::FromRow;

use crate::{storage::SupabaseStorage, AppState};

#[derive(Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: Option<String>, //nullable in auth.users table
}

pub async fn list_users(State(state): State<Arc<AppState>>) -> Json<Vec<User>> {
    let rows = sqlx::query_as!(User, "SELECT id, email FROM auth.users ORDER BY created_at DESC")
        .fetch_all(&state.db)
        .await
        .unwrap();

    Json(rows)
}

pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Json<String> {
    let storage = &state.storage;

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.file_name().unwrap().to_string();
        let data = field.bytes().await.unwrap();

        let _ = storage.upload_file(&name, data.to_vec()).await.unwrap();
        let url = storage.get_public_url(&name).await;
        return Json(url);
    }

    Json("No file uploaded".into())
}
