use axum::{Json, extract::{Multipart, Path, State}, http::StatusCode};
use serde::{Deserialize, Serialize};
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

async fn table_has_column(
    pool: &sqlx::Pool<sqlx::Postgres>,
    table: &str,
    column: &str,
) -> anyhow::Result<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = 'public' AND table_name = $1 AND column_name = $2)"
    )
    .bind(table)
    .bind(column)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

async fn table_column_names(
    pool: &sqlx::Pool<sqlx::Postgres>,
    table: &str,
) -> anyhow::Result<Vec<String>> {
    let cols = sqlx::query_scalar::<_, String>(
        "SELECT column_name FROM information_schema.columns WHERE table_schema = 'public' AND table_name = $1 ORDER BY ordinal_position"
    )
    .bind(table)
    .fetch_all(pool)
    .await?;
    Ok(cols)
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
    let columns = table_column_names(&state.db, &table).await.unwrap_or_default();
    let select_list = if columns.is_empty() {
        "r.*".to_string()
    } else {
        columns
            .iter()
            .map(|c| match c.as_str() {
                "created_at" | "updated_at" => format!("COALESCE(r.\"{}\", now()) AS \"{}\"", c, c),
                _ => format!("r.\"{}\"", c),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };

    let cte = format!(
        "WITH r AS (\n  SELECT * FROM json_populate_record(NULL::public.\"{}\", $1::json)\n)\n,\ninserted AS (\n  INSERT INTO public.\"{}\"\n  SELECT {} FROM r\n  RETURNING *\n)\nSELECT to_jsonb(inserted.*) AS row FROM inserted",
        quoted_table,
        quoted_table,
        select_list,
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
    let mut set_parts: Vec<String> = cols
        .iter()
        .map(|c| format!("\"{}\" = r.\"{}\"", c, c))
        .collect();
    let has_updated_at = table_has_column(&state.db, &table, "updated_at").await.unwrap_or(false);
    if has_updated_at && !cols.iter().any(|c| c == "updated_at") {
        set_parts.push("\"updated_at\" = now()".to_string());
    }
    let set_cols = set_parts.join(", ");

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

#[derive(Deserialize)]
pub struct CreateColorRequest {
    pub id: String,
    pub hex_value: Option<String>,
}

pub async fn create_color(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Json(req): Json<CreateColorRequest>,
) -> Result<Json<JsonValue>, (StatusCode, String)> {
    let id = req.id.trim().to_lowercase();
    if id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "color id is required".into()));
    }

    let hex_value = match &req.hex_value {
        Some(h) if !h.trim().is_empty() => {
            let h = h.trim();
            if h.len() != 7 || !h.starts_with('#') || !h[1..].chars().all(|c| c.is_ascii_hexdigit()) {
                return Err((StatusCode::BAD_REQUEST, "hex_value must be in #RRGGBB format".into()));
            }
            Some(h.to_string())
        }
        _ => None,
    };

    let row: (JsonValue,) = sqlx::query_as(
        "INSERT INTO colors (id, hex_value) VALUES ($1, $2) \
         ON CONFLICT (id) DO UPDATE SET id = colors.id \
         RETURNING to_jsonb(colors) AS row",
    )
    .bind(&id)
    .bind(&hex_value)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("could not create color: {e}")))?;

    Ok(Json(row.0))
}

#[derive(Deserialize)]
pub struct CreateCategoryRequest {
    pub name: String,
}

pub async fn create_category(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Json(req): Json<CreateCategoryRequest>,
) -> Result<Json<JsonValue>, (StatusCode, String)> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name is required".into()));
    }

    let row: (JsonValue,) = sqlx::query_as(
        r#"INSERT INTO "clothing-categories" (name) VALUES ($1)
           RETURNING to_jsonb("clothing-categories") AS row"#,
    )
    .bind(name)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("could not create category: {e}")))?;

    Ok(Json(row.0))
}

#[derive(Deserialize)]
pub struct CreateSubcategoryRequest {
    pub name: String,
    pub category_id: i64,
}

pub async fn create_subcategory(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Json(req): Json<CreateSubcategoryRequest>,
) -> Result<Json<JsonValue>, (StatusCode, String)> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name is required".into()));
    }

    let row: (JsonValue,) = sqlx::query_as(
        r#"INSERT INTO "clothing-subcategories" (name, category_id) VALUES ($1, $2)
           RETURNING to_jsonb("clothing-subcategories") AS row"#,
    )
    .bind(name)
    .bind(req.category_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("could not create subcategory: {e}")))?;

    Ok(Json(row.0))
}

#[derive(Deserialize)]
pub struct CreateOutfitRequest {
    pub date: String,
    pub notes: Option<String>,
    #[serde(default)]
    pub item_ids: Vec<String>,
}

const OUTFITS_WITH_ITEMS_SQL: &str =
    "SELECT to_jsonb(o) || jsonb_build_object('items', COALESCE((\n\
     SELECT jsonb_agg(to_jsonb(ci) ORDER BY ci.name)\n\
     FROM \"outfit-items\" oi\n\
     JOIN \"clothing-items\" ci ON ci.id = oi.clothing_item_id\n\
     WHERE oi.outfit_id = o.id\n\
     ), '[]'::jsonb)) AS row\n\
     FROM outfits o\n\
     WHERE o.user_id = $1";

pub async fn list_outfits(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
) -> Json<Vec<JsonValue>> {
    let sql = format!("{OUTFITS_WITH_ITEMS_SQL} ORDER BY o.date");
    let rows = sqlx::query(&sql)
        .bind(user_id)
        .fetch_all(&state.db)
        .await
        .unwrap();

    Json(rows.into_iter().map(|r| r.get::<JsonValue, _>("row")).collect())
}

pub async fn create_outfit(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Json(req): Json<CreateOutfitRequest>,
) -> Result<Json<JsonValue>, (StatusCode, String)> {
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let row: (JsonValue,) = sqlx::query_as(
        "INSERT INTO outfits (user_id, date, notes, is_active, created_at, updated_at) VALUES ($1, $2::timestamptz, $3, true, now(), now()) RETURNING to_jsonb(outfits) AS row",
    )
    .bind(user_id)
    .bind(&req.date)
    .bind(&req.notes)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("could not create outfit: {e}")))?;

    let outfit = row.0;
    let outfit_id = outfit
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    for item_id in &req.item_ids {
        sqlx::query(
            "INSERT INTO \"outfit-items\" (outfit_id, clothing_item_id) VALUES ($1::uuid, $2::uuid)",
        )
        .bind(&outfit_id)
        .bind(item_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("could not link item: {e}")))?;
    }

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let sql = format!("{OUTFITS_WITH_ITEMS_SQL} AND o.id::text = $2");
    let created: (JsonValue,) = sqlx::query_as(&sql)
        .bind(user_id)
        .bind(&outfit_id)
        .fetch_one(&state.db)
        .await
        .unwrap();

    Ok(Json(created.0))
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
