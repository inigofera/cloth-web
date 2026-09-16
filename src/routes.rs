use axum::{Json, extract::{Multipart, Path, State}, http::StatusCode};
use serde::Serialize;
use std::sync::Arc;
use serde_json::Value as JsonValue;
use sqlx::{FromRow, Row};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;

#[derive(Serialize, FromRow)]
pub struct ColumnInfo {
    pub table_name: String,
    pub column_name: String,
    pub data_type: String,
}

pub async fn list_tables(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
) -> Json<Vec<ColumnInfo>> {
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

async fn is_allowed_table(pool: &sqlx::Pool<sqlx::Postgres>, table: &str) -> anyhow::Result<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND table_name = $1)"
    )
    .bind(table)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

async fn table_has_user_id(pool: &sqlx::Pool<sqlx::Postgres>, table: &str) -> anyhow::Result<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = 'public' AND table_name = $1 AND column_name = 'user_id')"
    )
    .bind(table)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

fn quote_ident(table: &str) -> String {
    table.replace('"', "\"")
}

pub async fn get_table_rows(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
    AuthUser(user_id): AuthUser,
) -> Json<Vec<JsonValue>> {
    if !is_allowed_table(&state.db, &table).await.unwrap_or(false) {
        return Json(vec![]);
    }

    let quoted_table = quote_ident(&table);
    let has_user_id = table_has_user_id(&state.db, &table).await.unwrap_or(false);

    let rows = if has_user_id {
        let query = format!(
            "SELECT to_jsonb(t) AS row FROM public.\"{}\" t WHERE t.user_id = $1 ORDER BY 1 LIMIT 100",
            quoted_table
        );
        sqlx::query(&query).bind(user_id).fetch_all(&state.db).await.unwrap()
    } else {
        let query = format!(
            "SELECT to_jsonb(t) AS row FROM public.\"{}\" t ORDER BY 1 LIMIT 100",
            quoted_table
        );
        sqlx::query(&query).fetch_all(&state.db).await.unwrap()
    };

    let result: Vec<JsonValue> = rows
        .into_iter()
        .map(|r| r.get::<JsonValue, _>("row"))
        .collect();

    Json(result)
}

pub async fn insert_table_row(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
    AuthUser(user_id): AuthUser,
    Json(mut payload): Json<JsonValue>,
) -> Result<Json<Option<JsonValue>>, (StatusCode, String)> {
    if !is_allowed_table(&state.db, &table).await.unwrap_or(false) {
        return Err((StatusCode::NOT_FOUND, "table not found".into()));
    }
    if !table_has_user_id(&state.db, &table).await.unwrap_or(false) {
        return Err((StatusCode::FORBIDDEN, "table is shared reference data".into()));
    }

    // force ownership to the authenticated user, generate id if absent
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("user_id".into(), serde_json::to_value(user_id).unwrap());
        if !obj.contains_key("id") {
            obj.insert("id".into(), JsonValue::String(Uuid::new_v4().to_string()));
        }
    }

    let quoted_table = quote_ident(&table);
    let cte = format!(
        "WITH inserted AS (\n  INSERT INTO public.\"{}\"\n  SELECT * FROM json_populate_record(NULL::public.\"{}\", $1::json)\n  RETURNING *\n)\nSELECT to_jsonb(inserted.*) AS row FROM inserted",
        quoted_table,
        quoted_table,
    );

    let row: Option<(JsonValue,)> = sqlx::query_as(&cte)
        .bind(payload)
        .fetch_optional(&state.db)
        .await
        .unwrap();

    Ok(Json(row.map(|t| t.0)))
}

pub async fn delete_table_row(
    State(state): State<Arc<AppState>>,
    Path((table, id)): Path<(String, String)>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<Option<JsonValue>>, (StatusCode, String)> {
    if !is_allowed_table(&state.db, &table).await.unwrap_or(false) {
        return Err((StatusCode::NOT_FOUND, "table not found".into()));
    }
    if !table_has_user_id(&state.db, &table).await.unwrap_or(false) {
        return Err((StatusCode::FORBIDDEN, "table is shared reference data".into()));
    }

    let quoted_table = quote_ident(&table);
    let sql = format!(
        "WITH deleted AS (\n  DELETE FROM public.\"{}\" WHERE id::text = $1 AND user_id = $2 RETURNING *\n)\nSELECT to_jsonb(deleted.*) AS row FROM deleted",
        quoted_table
    );

    let row: Option<(JsonValue,)> = sqlx::query_as(&sql)
        .bind(id)
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
        .unwrap();

    Ok(Json(row.map(|t| t.0)))
}

pub async fn update_table_row(
    State(state): State<Arc<AppState>>,
    Path((table, id)): Path<(String, String)>,
    AuthUser(user_id): AuthUser,
    Json(payload): Json<JsonValue>,
) -> Result<Json<Option<JsonValue>>, (StatusCode, String)> {
    if !is_allowed_table(&state.db, &table).await.unwrap_or(false) {
        return Err((StatusCode::NOT_FOUND, "table not found".into()));
    }
    if !table_has_user_id(&state.db, &table).await.unwrap_or(false) {
        return Err((StatusCode::FORBIDDEN, "table is shared reference data".into()));
    }

    let obj = match payload.as_object() {
        Some(m) if !m.is_empty() => m,
        _ => return Ok(Json(None)),
    };

    // Build dynamic column list from provided keys (excluding id and user_id)
    let cols: Vec<String> = obj
        .keys()
        .filter(|k| k.as_str() != "id" && k.as_str() != "user_id")
        .map(|k| k.replace('"', "\\\""))
        .collect();

    if cols.is_empty() {
        return Ok(Json(None));
    }

    let quoted_table = quote_ident(&table);
    let set_cols = cols
        .iter()
        .map(|c| format!("\"{}\" = r.\"{}\"", c, c))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "WITH r AS (\n  SELECT * FROM json_populate_record(NULL::public.\"{}\", $2::json)\n)\nUPDATE public.\"{}\" AS t\nSET {}\nFROM r\nWHERE t.id::text = $1 AND t.user_id = $3\nRETURNING to_jsonb(t) AS row",
        quoted_table,
        quoted_table,
        set_cols
    );

    let row: Option<(JsonValue,)> = sqlx::query_as(&sql)
        .bind(id)
        .bind(payload)
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
        .unwrap();

    Ok(Json(row.map(|t| t.0)))
}

pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
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
