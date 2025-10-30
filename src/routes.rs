use axum::{Json, extract::{Multipart, State}};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;
use sqlx::FromRow;

use crate::AppState;

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

#[derive(Serialize, FromRow)]
pub struct ColumnInfo {
    pub table_name: String,
    pub column_name: String,
    pub data_type: String,
}

pub async fn list_tables(State(state): State<Arc<AppState>>) -> Json<Vec<ColumnInfo>> {
    let rows = sqlx::query_as::<_, ColumnInfo>(
        "SELECT t.table_name, c.column_name, c.data_type
         FROM information_schema.tables t
         JOIN information_schema.columns c 
           ON t.table_name = c.table_name AND t.table_schema = c.table_schema
         WHERE t.table_schema = 'public'
         ORDER BY t.table_name, c.ordinal_position"
    )
    .fetch_all(&state.db)
    .await
    .unwrap();

    Json(rows)
}

use axum::extract::Path;
use serde_json::Value as JsonValue;
use sqlx::Row;

async fn is_allowed_table(pool: &sqlx::Pool<sqlx::Postgres>, table: &str) -> anyhow::Result<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND table_name = $1)"
    )
    .bind(table)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

pub async fn get_table_rows(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
) -> Json<Vec<JsonValue>> {
    // validate table name against public schema
    if !is_allowed_table(&state.db, &table).await.unwrap_or(false) {
        return Json(vec![]);
    }

    // Build safe SQL using quoted identifier for the table name
    let query = format!(
        "SELECT to_jsonb(t) AS row FROM public.\"{}\" t ORDER BY 1 LIMIT 100",
        table.replace('"', "\"")
    );

    let rows = sqlx::query(&query)
        .fetch_all(&state.db)
        .await
        .unwrap();

    let result: Vec<JsonValue> = rows
        .into_iter()
        .map(|r| r.get::<JsonValue, _>("row"))
        .collect();

    Json(result)
}

pub async fn insert_table_row(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
    Json(payload): Json<JsonValue>,
) -> Json<Option<JsonValue>> {
    if !is_allowed_table(&state.db, &table).await.unwrap_or(false) {
        return Json(None);
    }

    // Use json_populate_record to coerce JSON into the table's record type
    let cte = format!(
        "WITH inserted AS (\n  INSERT INTO public.\"{}\"\n  SELECT * FROM json_populate_record(NULL::public.\"{}\", $1)\n  RETURNING *\n)\nSELECT to_jsonb(inserted.*) AS row FROM inserted",
        table.replace('"', "\""),
        table.replace('"', "\""),
    );

    let row: Option<(JsonValue,)> = sqlx::query_as(&cte)
        .bind(payload)
        .fetch_optional(&state.db)
        .await
        .unwrap();

    Json(row.map(|t| t.0))
}

pub async fn delete_table_row(
    State(state): State<Arc<AppState>>,
    Path((table, id)): Path<(String, String)>,
) -> Json<Option<JsonValue>> {
    if !is_allowed_table(&state.db, &table).await.unwrap_or(false) {
        return Json(None);
    }

    let sql = format!(
        "WITH deleted AS (\n  DELETE FROM public.\"{}\" WHERE id::text = $1 RETURNING *\n)\nSELECT to_jsonb(deleted.*) AS row FROM deleted",
        table.replace('"', "\\\"")
    );

    let row: Option<(JsonValue,)> = sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .unwrap();

    Json(row.map(|t| t.0))
}

pub async fn update_table_row(
    State(state): State<Arc<AppState>>,
    Path((table, id)): Path<(String, String)>,
    Json(payload): Json<JsonValue>,
) -> Json<Option<JsonValue>> {
    if !is_allowed_table(&state.db, &table).await.unwrap_or(false) {
        return Json(None);
    }

    let obj = match payload.as_object() {
        Some(m) if !m.is_empty() => m,
        _ => return Json(None),
    };

    // Build dynamic column list from provided keys (excluding id)
    let mut cols: Vec<String> = obj
        .keys()
        .filter(|k| k.as_str() != "id")
        .map(|k| k.replace('"', "\\\""))
        .collect();

    if cols.is_empty() {
        return Json(None);
    }

    let quoted_table = table.replace('"', "\\\"");
    let set_cols = cols
        .iter()
        .map(|c| format!("\"{}\" = r.\"{}\"", c, c))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "WITH r AS (\n  SELECT * FROM json_populate_record(NULL::public.\"{}\", $2)\n)\nUPDATE public.\"{}\" AS t\nSET {}\nFROM r\nWHERE t.id::text = $1\nRETURNING to_jsonb(t) AS row",
        quoted_table,
        quoted_table,
        set_cols
    );

    let row: Option<(JsonValue,)> = sqlx::query_as(&sql)
        .bind(id)
        .bind(payload)
        .fetch_optional(&state.db)
        .await
        .unwrap();

    Json(row.map(|t| t.0))
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
