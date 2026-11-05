use axum::{
    Json,
    extract::{Multipart, Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use serde_json::Value as JsonValue;
use sqlx::{FromRow, Row, Error as SqlxError};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::cache::CachedImage;

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
    // One schema read serves the whole app: the tables tab used to pay its own JOIN over
    // information_schema, the same questions the CRUD surface asked. Refreshing here keeps
    // the listing honest after a migration while the CRUD endpoints keep paying none.
    state.schema.refresh(&state.db).await;

    let facts_list = state.schema.snapshot();
    let mut rows: Vec<ColumnInfo> = vec![];
    for facts in facts_list {
        if is_generic_crud_table_blocked(&facts.name) {
            continue;
        }
        for column in facts.columns {
            rows.push(
                ColumnInfo {
                    table_name: facts.name.clone(),
                    column_name: column.column_name.clone(),
                    data_type: column.data_type.clone(),
                }
            );
        }
    }

    Json(rows)
}

pub fn is_generic_crud_table_blocked(table: &str) -> bool {
    table == "user-settings"
}


// The questions the CRUD surface used to ask Postgres per request - does this table exist,
// is it scoped to one user, which columns does it carry - are answered by the schema
// snapshot in AppState (see db.rs). Nothing here queries the schema any more; the shapes
// were rendered once per table at startup.

pub fn quote_ident(table: &str) -> String {
    table.replace('"', "\"")
}

fn db_write_error(error: SqlxError) -> (StatusCode, String) {
    if let SqlxError::Database(database_error) = &error {
        if database_error.code().as_deref() == Some("23514") {
            return (StatusCode::BAD_REQUEST, "value violates a field constraint".into());
        }
    }
    eprintln!("database write failed: {error}");
    (StatusCode::INTERNAL_SERVER_ERROR, "database write failed".into())
}

/// Hard cap for a single table read. Deliberately far beyond any realistic personal
/// wardrobe; truncation is surfaced to clients through `X-Total-Count` instead of
/// silently dropping rows (see UX_ANALYSIS.md, problem 1 / P1-B1).
const TABLE_ROWS_MAX_LIMIT: i64 = 10_000;

/// X-Total-Count is the server's own total for the query, counted before the page
/// was cut (`COUNT(*) OVER ()` folded into the list queries), so it is exact even
/// when a page is shorter than the total. A client that sees rows shorter than
/// this total knows the table was capped, not emptied (see UX_ANALYSIS.md, problem
/// 1 / P1-B1).
fn with_total_count(total: i64, rows: Vec<JsonValue>) -> (axum::http::HeaderMap, Json<Vec<JsonValue>>) {
    let mut headers = axum::http::HeaderMap::new();
    // An integer always parses as a valid header value; the if-let keeps this
    // panic-free without adding a dependency.
    if let Ok(value) = total.to_string().parse::<axum::http::header::HeaderValue>() {
        headers.insert("X-Total-Count", value);
    }
    (headers, Json(rows))
}

pub async fn get_table_rows(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
    AuthUser(user_id): AuthUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<(axum::http::HeaderMap, Json<Vec<JsonValue>>), (StatusCode, String)> {
    if is_generic_crud_table_blocked(&table) {
        return Err((
            StatusCode::FORBIDDEN,
            "Access user settings via /settings endpoint".to_string(),
        ));
    }

    // The table's shape comes from the snapshot built at startup: does it exist, is it
    // scoped to one user, which columns may a page order on. Asking Postgres these
    // questions per request was the cost this cache removes, so an unknown table is
    // answered from memory rather than a schema read before the 404 or empty page.
    let facts = state.schema.facts(&state.db, table.to_string()).await;
    match facts {
        None => return Ok(with_total_count(0, vec![])),
        Some(found) => {
            // Clients may request up to TABLE_ROWS_MAX_LIMIT rows, and may ask for
            // a page that starts anywhere: `offset` is a real row offset, so a client
            // can walk a big table page by page instead of paying for one capped dump
            // whose missing rows it cannot even name. The default is the cap itself: a
            // personal wardrobe will not come close to it, and anything beyond is made
            // visible through X-Total-Count rather than hidden.
            let limit = params
                .get("limit")
                .and_then(|v| v.parse::<i64>().ok())
                .filter(|l| *l >= 1)
                .map(|l| l.min(TABLE_ROWS_MAX_LIMIT))
                .unwrap_or(TABLE_ROWS_MAX_LIMIT);
            let offset = params
                .get("offset")
                .and_then(|v| v.parse::<i64>().ok())
                .filter(|o| *o >= 0)
                .unwrap_or(0);

            // One statement carries the rows and the count: `COUNT(*) OVER ()` runs after
            // the row filter but before the page is cut, so `X-Total-Count` reports the
            // rows that exist rather than the rows this page happens to include, and a
            // truncated page still tells the UI how much data was left out - without
            // scanning the same rows a second time for the total.
            let rows = match found.has_user_id {
                true => sqlx::query(&found.scoped_rows_sql)
                    .bind(user_id)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&state.db)
                    .await
                    .unwrap(),
                false => sqlx::query(&found.shared_rows_sql)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&state.db)
                    .await
                    .unwrap(),
            };

            let total: i64 = match rows.first() {
                None => 0,
                Some(row) => row.get::<i64, _>("total"),
            };

            let result: Vec<JsonValue> = rows
                .into_iter()
                .map(|r| r.get::<JsonValue, _>("row"))
                .collect();

            Ok(with_total_count(total, result))
        }
    }
}

pub async fn insert_table_row(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
    AuthUser(user_id): AuthUser,
    Json(mut payload): Json<JsonValue>,
) -> Result<Json<Option<JsonValue>>, (StatusCode, String)> {
    if is_generic_crud_table_blocked(&table) {
        return Err((
            StatusCode::FORBIDDEN,
            "Access user settings via /settings endpoint".to_string(),
        ));
    }

    // Existence, ownership, and the columns an insert may copy all come from the schema
    // snapshot taken at startup. A body that names a column the snapshot never heard of -
    // a migration that added one - costs that single request one schema read, rather than
    // every request paying three or four.
    let keys: Vec<String> = match payload.as_object() {
        None => vec![],
        Some(obj) => obj.keys().map(|k| k.to_string()).collect(),
    };
    let facts = state.schema.facts_for_write(&state.db, table.to_string(), &keys).await;
    let found = match facts {
        None => return Err((StatusCode::NOT_FOUND, "table not found".into())),
        Some(facts) => facts,
    };
    if !found.has_user_id {
        return Err((StatusCode::FORBIDDEN, "table is shared reference data".into()));
    }

    // Reject image_path values that point outside the user's own folder.
    if let Some(obj) = payload.as_object() {
        if let Some(image_path) = obj.get("image_path") {
            validate_image_path(image_path, &user_id)?;
        }
    }

    // force ownership to the authenticated user, generate id if absent
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("user_id".into(), serde_json::to_value(user_id).unwrap());
        if !obj.contains_key("id") {
            obj.insert("id".into(), JsonValue::String(Uuid::new_v4().to_string()));
        }
    }

    // The statement shape per table was decided at startup, so this request sends one
    // statement: the row is read back from the same insert, not re-queried after it.
    let row: Option<(JsonValue,)> = sqlx::query_as(&found.insert_sql)
        .bind(payload)
        .fetch_optional(&state.db)
        .await
        .map_err(db_write_error)?;

    Ok(Json(row.map(|t| t.0)))
}

pub async fn delete_table_row(
    State(state): State<Arc<AppState>>,
    Path((table, id)): Path<(String, String)>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<Option<JsonValue>>, (StatusCode, String)> {
    if is_generic_crud_table_blocked(&table) {
        return Err((
            StatusCode::FORBIDDEN,
            "Access user settings via /settings endpoint".to_string(),
        ));
    }

    // Outfit deletion must go through DELETE /outfits/:id so wear counts stay in sync.
    if table == "outfits" {
        return Err((
            StatusCode::METHOD_NOT_ALLOWED,
            "use DELETE /outfits/:id to delete outfits".into(),
        ));
    }
    // Existence and ownership come from the startup snapshot, and the statement was
    // rendered once per table. A uuid-keyed table compares its id column against a uuid
    // parameter, so the primary-key index answers the lookup rather than casting the
    // column on every row it walked.
    let facts = state.schema.facts(&state.db, table.to_string()).await;
    let found = match facts {
        None => return Err((StatusCode::NOT_FOUND, "table not found".into())),
        Some(facts) => facts,
    };
    if !found.has_user_id {
        return Err((StatusCode::FORBIDDEN, "table is shared reference data".into()));
    }

    let row: Option<(JsonValue,)> = match found.id_kind.as_str() {
        "uuid" => match Uuid::parse_str(&id) {
            // An id that is not a uuid cannot name a uuid-keyed row: the answer is no row,
            // the same answer a lookup that matched nothing used to give.
            Err(_) => None,
            Ok(target) => sqlx::query_as(&found.delete_sql)
                .bind(target)
                .bind(user_id)
                .fetch_optional(&state.db)
                .await
                .unwrap(),
        },
        _ => sqlx::query_as(&found.delete_sql)
            .bind(id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await
            .unwrap(),
    };

    Ok(Json(row.map(|t| t.0)))
}

pub async fn update_table_row(
    State(state): State<Arc<AppState>>,
    Path((table, id)): Path<(String, String)>,
    AuthUser(user_id): AuthUser,
    Json(payload): Json<JsonValue>,
) -> Result<Json<Option<JsonValue>>, (StatusCode, String)> {
    if is_generic_crud_table_blocked(&table) {
        return Err((
            StatusCode::FORBIDDEN,
            "Access user settings via /settings endpoint".to_string(),
        ));
    }

    // Existence, ownership, and whether this table even has an `updated_at` column all
    // come from the snapshot: no schema round trip on the way to the write.
    let facts = state.schema.facts(&state.db, table.to_string()).await;
    let found = match facts {
        None => return Err((StatusCode::NOT_FOUND, "table not found".into())),
        Some(facts) => facts,
    };
    if !found.has_user_id {
        return Err((StatusCode::FORBIDDEN, "table is shared reference data".into()));
    }

    let obj = match payload.as_object() {
        Some(m) if !m.is_empty() => m,
        _ => return Ok(Json(None)),
    };

    // Reject image_path values that point outside the user's own folder.
    if let Some(image_path) = obj.get("image_path") {
        validate_image_path(image_path, &user_id)?;
    }

    // Build dynamic column list from provided keys (excluding id and user_id)
    let cols: Vec<String> = obj
        .keys()
        .filter(|k| k.as_str() != "id" && k.as_str() != "user_id")
        .map(|k| k.replace('"', "\\\""))
        .collect();

    if cols.is_empty() {
        return Ok(Json(None));
    }

    let mut set_parts: Vec<String> = cols
        .iter()
        .map(|c| format!("\"{}\" = r.\"{}\"", c, c))
        .collect();
    if found.has_updated_at && !cols.iter().any(|c| c == "updated_at") {
        set_parts.push("\"updated_at\" = now()".to_string());
    }
    let set_cols = set_parts.join(", ");

    // Head and tail were rendered for this table at startup; only the columns this request
    // asked for are written between them.
    let update_head = found.update_head;
    let update_tail = found.update_tail;
    let sql = format!("{update_head}{set_cols}{update_tail}");

    let row: Option<(JsonValue,)> = match found.id_kind.as_str() {
        "uuid" => match Uuid::parse_str(&id) {
            Err(_) => None,
            Ok(target) => sqlx::query_as(&sql)
                .bind(target)
                .bind(payload)
                .bind(user_id)
                .fetch_optional(&state.db)
                .await
                .map_err(db_write_error)?,
        },
        _ => sqlx::query_as(&sql)
            .bind(id)
            .bind(payload)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await
            .map_err(db_write_error)?,
    };

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
    pub image_path: Option<String>,
}

// The outfit query in two named pieces: the columns it reads, and the rows it reads them
// from. Callers used to bolt an extra clause onto one long string, which silently relied on
// where that string happened to end; named pieces make the two shapes explicit - a page and
// a single outfit - and let the page carry its own total.
//
// The page carries an outfit's item ids and nothing else. The full clothing row used to be
// copied into every outfit that names it, so an item worn 50 times crossed the wire 50 times
// with all of its fields; the ids keep the join (`ORDER BY ci.name`) the view renders in,
// while the single copy of each item the client already holds does the rest.
const OUTFITS_ROW_EXPR: &str =
    "SELECT to_jsonb(o) || jsonb_build_object(\n\
     'date', to_char(o.date, 'YYYY-MM-DD'),\n\
     'item_ids', COALESCE((\n\
     SELECT jsonb_agg(oi.clothing_item_id ORDER BY ci.name)\n\
     FROM \"outfit-items\" oi\n\
     JOIN \"clothing-items\" ci ON ci.id = oi.clothing_item_id\n\
     WHERE oi.outfit_id = o.id\n\
     ), '[]'::jsonb)) AS row";

const OUTFITS_FROM_SCOPED: &str = "FROM outfits o WHERE o.user_id = $1";

pub async fn list_outfits(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Query(params): Query<HashMap<String, String>>,
) -> (axum::http::HeaderMap, Json<Vec<JsonValue>>) {
    // Paging and the total in one statement: `COUNT(*) OVER ()` counts the rows the
    // filter matched before the page is cut, so the completeness signal costs no second
    // pass over the join that builds the ids, and a client may ask to start anywhere.
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|l| *l >= 1)
        .map(|l| l.min(TABLE_ROWS_MAX_LIMIT))
        .unwrap_or(TABLE_ROWS_MAX_LIMIT);
    let offset = params
        .get("offset")
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|o| *o >= 0)
        .unwrap_or(0);

    let sql = format!(
        "{OUTFITS_ROW_EXPR}, COUNT(*) OVER () AS total {OUTFITS_FROM_SCOPED} ORDER BY o.date LIMIT $2 OFFSET $3",
    );
    let rows = sqlx::query(&sql)
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap();

    let total: i64 = match rows.first() {
        None => 0,
        Some(row) => row.get::<i64, _>("total"),
    };

    with_total_count(
        total,
        rows.into_iter().map(|r| r.get::<JsonValue, _>("row")).collect(),
    )
}

pub async fn create_outfit(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Json(req): Json<CreateOutfitRequest>,
) -> Result<Json<JsonValue>, (StatusCode, String)> {
    if let Some(ref path) = req.image_path {
        validate_image_path(&serde_json::to_value(path).unwrap(), &user_id)?;
    }

    // Resolve the ids this request names before building any SQL, and drop repeats: the
    // statement below carries uuids this server produced, never text the client chose, and
    // one outfit cannot point at the same item twice.
    if req.item_ids.len() > 200 {
        return Err((StatusCode::BAD_REQUEST, "an outfit cannot carry more than 200 items".to_string()));
    }

    let mut item_literals: Vec<String> = vec![];
    for raw_id in &req.item_ids {
        match Uuid::parse_str(raw_id) {
            Err(_) => return Err((StatusCode::BAD_REQUEST, "item id is not a uuid".to_string())),
            Ok(item_uuid) => {
                if !item_literals.iter().any(|seen| *seen == item_uuid.to_string()) {
                    item_literals.push(item_uuid.to_string());
                }
            },
        }
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let row: (JsonValue,) = sqlx::query_as(
        "INSERT INTO outfits (user_id, date, notes, image_path, is_active, created_at, updated_at) VALUES ($1, $2::timestamptz, $3, $4, true, now(), now()) RETURNING to_jsonb(outfits) AS row",
    )
    .bind(user_id)
    .bind(&req.date)
    .bind(&req.notes)
    .bind(&req.image_path)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("could not create outfit: {e}")))?;


    let outfit = row.0;
    let outfit_id = outfit
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    // Every link this request asks for goes over the wire in one statement, rather than
    // one round trip per item: an outfit with a dozen pieces used to pay a dozen queries.
    if !item_literals.is_empty() {
        let pairs: Vec<String> = item_literals.iter().map(|literal| format!("($1::uuid, '{}')", literal)).collect();
        let link_sql = format!("INSERT INTO \"outfit-items\" (outfit_id, clothing_item_id) VALUES {}", pairs.join(", "));
        sqlx::query(&link_sql)
        .bind(&outfit_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("could not link items: {e}")))?;
    }

    sqlx::query(
        "UPDATE \"clothing-items\" ci \
         SET wear_count = COALESCE(ci.wear_count, 0) + 1, updated_at = now() \
         FROM (SELECT DISTINCT oi.clothing_item_id FROM \"outfit-items\" oi WHERE oi.outfit_id = $1::uuid) s \
         WHERE ci.id = s.clothing_item_id AND ci.user_id = $2",
    )
    .bind(&outfit_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("could not update wear counts: {e}")))?;

    // The outfit the caller gets back is read from inside the same transaction that made it,
    // so the links above are already visible to it; no second round trip after the commit,
    // and no window where the outfit exists but its items do not.
    let created_sql = format!("{OUTFITS_ROW_EXPR} {OUTFITS_FROM_SCOPED} AND o.id = $2");
    let created: (JsonValue,) = sqlx::query_as(&created_sql)
    .bind(user_id)
    .bind(&outfit_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("could not read back the outfit: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(created.0))
}

pub async fn delete_outfit(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(outfit_id): Path<String>,
) -> Result<Json<JsonValue>, (StatusCode, String)> {
    let outfit_uuid = Uuid::parse_str(&outfit_id)
        .map_err(|_| (StatusCode::NOT_FOUND, "outfit not found".into()))?;

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query(
        "UPDATE \"clothing-items\" ci \
         SET wear_count = GREATEST(COALESCE(ci.wear_count, 0) - 1, 0), updated_at = now() \
         FROM (SELECT DISTINCT oi.clothing_item_id FROM \"outfit-items\" oi WHERE oi.outfit_id = $1::uuid) s \
         WHERE ci.id = s.clothing_item_id AND ci.user_id = $2",
    )
    .bind(outfit_uuid)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("could not update wear counts: {e}")))?;

    sqlx::query("DELETE FROM \"outfit-items\" WHERE outfit_id = $1")
        .bind(outfit_uuid)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let row: Option<(JsonValue,)> = sqlx::query_as(
        "DELETE FROM outfits WHERE id = $1 AND user_id = $2 RETURNING to_jsonb(outfits) AS row",
    )
    .bind(outfit_uuid)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match row {
        Some(r) => Ok(Json(r.0)),
        None => Err((StatusCode::NOT_FOUND, "outfit not found".into())),
    }
}

pub async fn sync_discord_outfits(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<crate::discord::SyncResult>, (StatusCode, String)> {
    let result = crate::discord::sync_discord_for_user(&state.db, user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(result))
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct UserSettingsResponse {
    pub tryon_provider: String,
    pub has_api_key: bool,
    pub api_key_hint: Option<String>,
    pub local_qwen_url: String,
    pub reference_image_path: Option<String>,
    pub discord_user_id: Option<String>,
}

#[derive(Serialize)]
pub struct DiscordLinkCodeResponse {
    pub code: String,
    pub expires_in_seconds: u32,
}

#[derive(Deserialize)]
pub struct UpdateSettingsRequest {
    pub tryon_provider: Option<String>,
    pub api_key: Option<String>,
    pub local_qwen_url: Option<String>,
    pub reference_image_path: Option<String>,
}

pub fn validate_local_qwen_url(raw_url: &str) -> Result<reqwest::Url, String> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return Err("Local Qwen URL cannot be empty".to_string());
    }

    let parsed = reqwest::Url::parse(trimmed)
        .map_err(|e| format!("Invalid URL: {e}"))?;

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err("URL scheme must be http or https".to_string());
    }

    let host = parsed.host_str().ok_or_else(|| "URL must have a host".to_string())?;

    // Block link-local addresses (e.g. AWS/GCP/Azure cloud metadata 169.254.x.x)
    if host == "169.254.169.254" || host.starts_with("169.254.") {
        return Err("Link-local addresses (metadata services) are prohibited".to_string());
    }

    // Default ports: if None, 80 for http, 443 for https
    let port = parsed.port_or_known_default().unwrap_or(80);
    if port == 3000 || port == 5432 {
        return Err(format!("Port {port} is reserved and prohibited"));
    }

    let allow_lan = std::env::var("ALLOW_LAN_INFERENCE_URLS")
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let is_loopback = host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1";

    if is_loopback {
        return Ok(parsed);
    }

    if allow_lan {
        // Check RFC 1918 private LAN addresses
        let is_lan = if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            match ip {
                std::net::IpAddr::V4(ipv4) => {
                    let octets = ipv4.octets();
                    octets[0] == 10
                        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                        || (octets[0] == 192 && octets[1] == 168)
                }
                std::net::IpAddr::V6(_) => false,
            }
        } else {
            false
        };

        if is_lan {
            return Ok(parsed);
        }
    }

    Err(
        "Host must be 127.0.0.1 or localhost (or LAN IP if ALLOW_LAN_INFERENCE_URLS=true)"
            .to_string(),
    )
}

pub async fn get_user_settings(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<UserSettingsResponse>, (StatusCode, String)> {
    let row: Option<(
        Option<Vec<u8>>,
        Option<Vec<u8>>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"SELECT encrypted_api_key, api_key_nonce, reference_image_path, tryon_provider, local_qwen_url, discord_user_id
           FROM public."user-settings"
           WHERE user_id = $1"#,
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch settings: {e}")))?;

    let (has_api_key, api_key_hint, reference_image_path, tryon_provider, local_qwen_url, discord_user_id) = match row {
        Some((Some(enc), Some(nonce), ref_path, provider, qwen_url, disc_id)) if !enc.is_empty() && !nonce.is_empty() => {
            let provider = provider.unwrap_or_else(|| "gemini".to_string());
            let qwen_url = qwen_url.unwrap_or_else(|| "http://127.0.0.1:8001".to_string());
            match crate::crypto::decrypt_api_key(&enc, &nonce, &state.encryption_key) {
                Ok(plain) => {
                    let hint = if plain.chars().count() >= 4 {
                        let last4: String = plain.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
                        format!("...{last4}")
                    } else {
                        format!("...{plain}")
                    };
                    drop(plain);
                    (true, Some(hint), ref_path, provider, qwen_url, disc_id)
                }
                Err(e) => {
                    eprintln!("Failed to decrypt API key for user {user_id}: {e}");
                    (false, None, ref_path, provider, qwen_url, disc_id)
                }
            }
        }
        Some((_, _, ref_path, provider, qwen_url, disc_id)) => (
            false,
            None,
            ref_path,
            provider.unwrap_or_else(|| "gemini".to_string()),
            qwen_url.unwrap_or_else(|| "http://127.0.0.1:8001".to_string()),
            disc_id,
        ),
        None => (false, None, None, "gemini".to_string(), "http://127.0.0.1:8001".to_string(), None),
    };

    Ok(Json(UserSettingsResponse {
        tryon_provider,
        has_api_key,
        api_key_hint,
        local_qwen_url,
        reference_image_path,
        discord_user_id,
    }))
}

pub async fn update_user_settings(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<UserSettingsResponse>, (StatusCode, String)> {
    let (update_api_key, new_enc, new_nonce) = match payload.api_key {
        Some(ref key) => {
            let trimmed = key.trim();
            if trimmed.is_empty() {
                (true, None, None)
            } else {
                if trimmed.len() < 10 || trimmed.len() > 256 {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        "API key must be between 10 and 256 characters".to_string(),
                    ));
                }
                let (enc, nonce) = crate::crypto::encrypt_api_key(trimmed, &state.encryption_key)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("encryption error: {e}")))?;
                (true, Some(enc), Some(nonce))
            }
        }
        None => (false, None, None),
    };

    let (update_ref_path, new_ref_path) = match payload.reference_image_path {
        Some(ref path) => {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                (true, None)
            } else {
                validate_image_path(&serde_json::to_value(trimmed).unwrap(), &user_id)?;
                (true, Some(trimmed.to_string()))
            }
        }
        None => (false, None),
    };

    let (update_provider, new_provider) = match payload.tryon_provider {
        Some(ref p) => {
            let trimmed = p.trim().to_lowercase();
            if trimmed != "gemini" && trimmed != "qwen_local" {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "tryon_provider must be either 'gemini' or 'qwen_local'".to_string(),
                ));
            }
            (true, Some(trimmed))
        }
        None => (false, None),
    };

    let (update_qwen_url, new_qwen_url) = match payload.local_qwen_url {
        Some(ref u) => {
            let parsed = validate_local_qwen_url(u)
                .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            (true, Some(parsed.to_string().trim_end_matches('/').to_string()))
        }
        None => (false, None),
    };

    sqlx::query(
        r#"INSERT INTO public."user-settings" (user_id, encrypted_api_key, api_key_nonce, reference_image_path, tryon_provider, local_qwen_url, updated_at)
           VALUES ($1, $2, $3, $4, COALESCE($7, 'gemini'), COALESCE($9, 'http://127.0.0.1:8001'), now())
           ON CONFLICT (user_id) DO UPDATE SET
               encrypted_api_key = CASE WHEN $5 THEN EXCLUDED.encrypted_api_key ELSE public."user-settings".encrypted_api_key END,
               api_key_nonce = CASE WHEN $5 THEN EXCLUDED.api_key_nonce ELSE public."user-settings".api_key_nonce END,
               reference_image_path = CASE WHEN $6 THEN EXCLUDED.reference_image_path ELSE public."user-settings".reference_image_path END,
               tryon_provider = CASE WHEN $8 THEN EXCLUDED.tryon_provider ELSE public."user-settings".tryon_provider END,
               local_qwen_url = CASE WHEN $10 THEN EXCLUDED.local_qwen_url ELSE public."user-settings".local_qwen_url END,
               updated_at = now()"#,
    )
    .bind(user_id)
    .bind(new_enc)
    .bind(new_nonce)
    .bind(new_ref_path)
    .bind(update_api_key)
    .bind(update_ref_path)
    .bind(new_provider)
    .bind(update_provider)
    .bind(new_qwen_url)
    .bind(update_qwen_url)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to update settings: {e}")))?;

    get_user_settings(State(state), AuthUser(user_id)).await
}

pub async fn generate_discord_link_code(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<DiscordLinkCodeResponse>, (StatusCode, String)> {
    use rand::Rng;
    let code: u32 = rand::thread_rng().gen_range(100000..999999);
    let code_str = code.to_string();

    sqlx::query(
        r#"INSERT INTO public."user-settings" (user_id, discord_link_code, discord_link_expires_at, updated_at)
           VALUES ($1, $2, now() + interval '15 minutes', now())
           ON CONFLICT (user_id) DO UPDATE SET
               discord_link_code = EXCLUDED.discord_link_code,
               discord_link_expires_at = EXCLUDED.discord_link_expires_at,
               updated_at = now()"#,
    )
    .bind(user_id)
    .bind(&code_str)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to generate code: {e}")))?;

    Ok(Json(DiscordLinkCodeResponse {
        code: code_str,
        expires_in_seconds: 900,
    }))
}

pub async fn unlink_discord_account(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<UserSettingsResponse>, (StatusCode, String)> {
    sqlx::query(
        r#"UPDATE public."user-settings"
           SET discord_user_id = NULL, discord_link_code = NULL, discord_link_expires_at = NULL, updated_at = now()
           WHERE user_id = $1"#,
    )
    .bind(user_id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to unlink Discord: {e}")))?;

    get_user_settings(State(state), AuthUser(user_id)).await
}

#[derive(Deserialize)]
pub struct TestQwenParams {
    pub url: Option<String>,
}

pub async fn test_qwen_connection(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Query(params): Query<TestQwenParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let target_url = match params.url {
        Some(u) if !u.trim().is_empty() => {
            validate_local_qwen_url(&u).map_err(|e| (StatusCode::BAD_REQUEST, e))?.to_string()
        }
        _ => {
            let row: Option<(Option<String>,)> = sqlx::query_as(
                r#"SELECT local_qwen_url FROM public."user-settings" WHERE user_id = $1"#,
            )
            .bind(user_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to query settings: {e}")))?;

            let saved_url = row.and_then(|r| r.0).unwrap_or_else(|| "http://127.0.0.1:8001".to_string());
            validate_local_qwen_url(&saved_url).map_err(|e| (StatusCode::BAD_REQUEST, e))?.to_string()
        }
    };

    let base = target_url.trim_end_matches('/');
    let health_url = format!("{base}/health");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let res = client
        .get(&health_url)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("Failed to connect to local Qwen server at {health_url}: {e}"),
            )
        })?;

    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("Local Qwen server returned error ({status}): {text}"),
        ));
    }

    let json: serde_json::Value = res
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Invalid JSON from local Qwen server: {e}")))?;

    Ok(Json(json))
}

pub fn build_qwen_prompt(items: &[GarmentItemRow]) -> String {
    let mut prompt = String::from(
        "Keep the person's identity, face, hair, pose, body proportions, and background from Picture 1 unchanged. \
         Replace the clothes worn by the person in Picture 1 with the garments shown in the following pictures:\n"
    );

    for (idx, item) in items.iter().enumerate() {
        let cat = item.category_name.as_deref().unwrap_or("Uncategorized");
        let subcat = item.subcategory_name.as_deref().unwrap_or("General");
        let color = item.color_id.as_deref().unwrap_or("Unspecified");
        let name = &item.name;
        prompt.push_str(&format!(
            "- Picture {}: {} ({}) in {}, \"{}\"\n",
            idx + 2, cat, subcat, color, name
        ));
    }

    prompt.push_str(
        "Remove any existing clothing in Picture 1 that conflicts with these garments. \
         Ensure natural fabric drape, realistic lighting, and accurate garment details."
    );

    prompt
}

#[derive(Deserialize)]
pub struct GeneratePreviewRequest {
    pub item_ids: Vec<Uuid>,
}

#[derive(Serialize)]
pub struct GeneratePreviewResponse {
    pub image_path: String,
}

#[derive(FromRow, Clone, Debug)]
pub struct GarmentItemRow {
    pub id: Uuid,
    pub name: String,
    pub color_id: Option<String>,
    pub image_path: Option<String>,
    #[allow(dead_code)]
    pub notes: Option<String>,
    pub category_name: Option<String>,
    pub subcategory_name: Option<String>,
}

pub async fn generate_outfit_preview(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Json(payload): Json<GeneratePreviewRequest>,
) -> Result<Json<GeneratePreviewResponse>, (StatusCode, String)> {
    if payload.item_ids.is_empty() || payload.item_ids.len() > 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            "item_ids must contain between 1 and 8 items".to_string(),
        ));
    }

    let settings_row: Option<(
        Option<Vec<u8>>,
        Option<Vec<u8>>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"SELECT encrypted_api_key, api_key_nonce, reference_image_path, tryon_provider, local_qwen_url
           FROM public."user-settings"
           WHERE user_id = $1"#,
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to query settings: {e}")))?;

    let reference_image_path = match settings_row.as_ref().and_then(|r| r.2.as_ref()) {
        Some(p) if !p.trim().is_empty() => p.clone(),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Full-body reference photo is not configured in Settings.".to_string(),
            ));
        }
    };

    let tryon_provider = settings_row
        .as_ref()
        .and_then(|r| r.3.as_deref())
        .unwrap_or("gemini")
        .to_string();

    let local_qwen_url = settings_row
        .as_ref()
        .and_then(|r| r.4.as_deref())
        .unwrap_or("http://127.0.0.1:8001")
        .to_string();

    let gemini_api_key = if tryon_provider == "gemini" {
        let (encrypted_api_key, api_key_nonce) = match settings_row.as_ref() {
            Some((Some(enc), Some(nonce), _, _, _)) if !enc.is_empty() && !nonce.is_empty() => {
                (enc, nonce)
            }
            _ => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Gemini API key is not configured in Settings.".to_string(),
                ));
            }
        };
        let key = crate::crypto::decrypt_api_key(encrypted_api_key, api_key_nonce, &state.encryption_key)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decrypt API key: {e}")))?;
        Some(key)
    } else {
        validate_local_qwen_url(&local_qwen_url)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid local Qwen URL: {e}")))?;
        None
    };

    let items = sqlx::query_as::<_, GarmentItemRow>(
        r#"SELECT 
            ci.id,
            ci.name,
            ci.color_id,
            ci.image_path,
            ci.notes,
            cc.name AS category_name,
            cs.name AS subcategory_name
        FROM public."clothing-items" ci
        LEFT JOIN public."clothing-categories" cc ON ci.category_id = cc.id
        LEFT JOIN public."clothing-subcategories" cs ON ci.subcategory_id = cs.id
        WHERE ci.id = ANY($1) AND ci.user_id = $2"#,
    )
    .bind(&payload.item_ids)
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to query items: {e}")))?;

    for req_id in &payload.item_ids {
        if !items.iter().any(|item| item.id == *req_id) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Item with id '{req_id}' was not found in your wardrobe."),
            ));
        }
    }

    for item in &items {
        match &item.image_path {
            Some(path) if !path.trim().is_empty() => {}
            _ => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("Item '{}' does not have an uploaded photo.", item.name),
                ));
            }
        }
    }

    let ref_key = extract_object_path(&reference_image_path);
    let mut download_futures = Vec::new();
    download_futures.push(state.storage.download_file(ref_key));
    for item in &items {
        let item_key = extract_object_path(item.image_path.as_deref().unwrap());
        download_futures.push(state.storage.download_file(item_key));
    }

    let downloaded_images = futures::future::try_join_all(download_futures)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to download images from storage: {e}")))?;

    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;

    let (ref_bytes, ref_ct) = &downloaded_images[0];
    let ref_mime = content_type_for(ref_key, ref_ct);
    let ref_b64 = BASE64.encode(ref_bytes);

    let (ref_w, ref_h) = match image::load_from_memory(ref_bytes) {
        Ok(img) => {
            let oriented = crate::upload::apply_orientation(img.to_rgba8(), crate::upload::exif_orientation(ref_bytes));
            oriented.dimensions()
        }
        Err(_) => (1024, 1024),
    };
    let chosen_aspect_ratio = crate::upload::best_supported_aspect_ratio(ref_w, ref_h);

    let raw_bytes = if tryon_provider == "qwen_local" {
        let qwen_prompt = build_qwen_prompt(&items);
        let mut images_b64 = vec![ref_b64];
        for (idx, _) in items.iter().enumerate() {
            let (item_bytes, _) = &downloaded_images[idx + 1];
            images_b64.push(BASE64.encode(item_bytes));
        }

        let qwen_width = std::env::var("QWEN_WIDTH")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(832);
        let qwen_height = std::env::var("QWEN_HEIGHT")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(1248);

        let qwen_request = serde_json::json!({
            "prompt": qwen_prompt,
            "images_base64": images_b64,
            "width": qwen_width,
            "height": qwen_height,
            "seed": 42
        });

        let timeout_secs = std::env::var("QWEN_TIMEOUT_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let gen_endpoint = format!("{}/generate", local_qwen_url.trim_end_matches('/'));
        let res = client
            .post(&gen_endpoint)
            .json(&qwen_request)
            .send()
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to reach local Qwen server at {gen_endpoint}: {e}")))?;

        let status = res.status();
        if !status.is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err((StatusCode::BAD_GATEWAY, format!("Local Qwen server error ({status}): {err_text}")));
        }

        let res_json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Invalid response from local Qwen server: {e}")))?;

        let b64 = res_json
            .get("image_base64")
            .and_then(|v| v.as_str())
            .ok_or_else(|| (StatusCode::BAD_GATEWAY, "Local Qwen server response missing 'image_base64'".to_string()))?;

        BASE64
            .decode(b64)
            .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to decode image from local Qwen server: {e}")))?
    } else {
        let api_key = gemini_api_key.expect("Gemini API key verified above");
        let mut parts: Vec<serde_json::Value> = Vec::new();

        parts.push(serde_json::json!({
            "text": format!(
                "Task: Photorealistic virtual try-on.\n\
                 You are dressing the real person shown in the Target User Reference Photo below.\n\n\
                 [TARGET USER REFERENCE PHOTO]:"
            )
        }));

        parts.push(serde_json::json!({
            "inlineData": {
                "mimeType": ref_mime,
                "data": ref_b64
            }
        }));

        parts.push(serde_json::json!({
            "text": format!(
                "CRITICAL SUBJECT PRESERVATION RULES:\n\
                 - The person in the output image MUST BE the exact same person from the Target User Reference Photo above.\n\
                 - Preserve their full head, face, facial hair, hairstyle, eyes, nose, mouth, skin tone, body proportions, posture, and mirror selfie pose.\n\
                 - DO NOT CROP THE HEAD. The user's entire head, face, and hair MUST be clearly visible and recognizable.\n\
                 - DO NOT replace this person with any generic model or headless mannequin.\n\
                 - Preserve the same camera framing, aspect ratio ({chosen_aspect_ratio}), orientation, and background setting from the Target User Reference Photo.\n\n\
                 Now, here are the new garments to dress this person in:\n"
            )
        }));

        for (idx, item) in items.iter().enumerate() {
            let (item_bytes, item_ct) = &downloaded_images[idx + 1];
            let item_key = extract_object_path(item.image_path.as_deref().unwrap());
            let item_mime = content_type_for(item_key, item_ct);
            let item_b64 = BASE64.encode(item_bytes);

            let cat = item.category_name.as_deref().unwrap_or("Uncategorized");
            let subcat = item.subcategory_name.as_deref().unwrap_or("General");
            let color = item.color_id.as_deref().unwrap_or("Unspecified");
            let name = &item.name;

            parts.push(serde_json::json!({
                "text": format!(
                    "[GARMENT {} - Category: {} / {}, Color: {}, Name: \"{}\"]:\n\
                     Extract ONLY the {} from this photo and dress the Target User in it.\n\
                     NOTE: This garment photo may show a catalog model, mannequin, or studio background. \
                     You MUST COMPLETELY IGNORE any model, face, or background in this garment photo. \
                     Under no circumstances should the output image be headless or on a white studio background.",
                    idx + 1, cat, subcat, color, name, name
                )
            }));

            parts.push(serde_json::json!({
                "inlineData": {
                    "mimeType": item_mime,
                    "data": item_b64
                }
            }));
        }

        parts.push(serde_json::json!({
            "text": format!(
                "FINAL OUTPUT REQUIREMENTS:\n\
                 - Output a single photorealistic image of the TARGET USER from the Reference Photo, head-to-toe with their FULL HEAD, FACE, and HAIR visible, wearing the specified new garments.\n\
                 - Completely remove any previous conflicting clothing from the user in the Reference Photo.\n\
                 - Ensure seamless realistic fabric drape, accurate garment colors, and lighting matching the original Reference Photo.\n\
                 - Aspect ratio MUST be {chosen_aspect_ratio}.\n\
                 - Output ONLY the final generated image."
            )
        }));

        let request_body = serde_json::json!({
            "contents": [{
                "parts": parts
            }],
            "generationConfig": {
                "responseModalities": ["IMAGE"],
                "imageConfig": {
                    "aspectRatio": chosen_aspect_ratio
                }
            }
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let res = client
            .post("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent")
            .header("x-goog-api-key", &api_key)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to reach Gemini API: {e}")))?;

        let status = res.status();
        if !status.is_success() {
            let err_text = res.text().await.unwrap_or_default();
            let error_msg = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&err_text) {
                if let Some(msg) = parsed.pointer("/error/message").and_then(|v| v.as_str()) {
                    msg.to_string()
                } else {
                    err_text
                }
            } else {
                err_text
            };

            let mapped_status = if status.is_client_error() {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::BAD_GATEWAY
            };
            return Err((mapped_status, format!("Gemini API error ({status}): {error_msg}")));
        }

        let res_json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Invalid response from Gemini API: {e}")))?;

        let candidates = res_json.get("candidates").and_then(|c| c.as_array());
        let parts = candidates
            .and_then(|c| c.first())
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array());

        let base64_image = parts.and_then(|parts| {
            parts.iter().find_map(|part| {
                part.get("inlineData")
                    .or_else(|| part.get("inline_data"))
                    .and_then(|data_obj| data_obj.get("data"))
                    .and_then(|data_val| data_val.as_str())
            })
        });

        let base64_image = match base64_image {
            Some(b64) => b64,
            None => {
                let finish_reason = candidates
                    .and_then(|c| c.first())
                    .and_then(|c| c.get("finishReason"))
                    .and_then(|r| r.as_str())
                    .unwrap_or("UNKNOWN");
                return Err((
                    StatusCode::BAD_GATEWAY,
                    format!("Gemini did not return an image. Finish reason: {finish_reason}"),
                ));
            }
        };

        BASE64
            .decode(base64_image)
            .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to decode image from Gemini: {e}")))?
    };

    let (processed_bytes, _) = crate::upload::process_and_encode_image_matching_aspect(
        &raw_bytes,
        ref_w,
        ref_h,
    )?;

    let key = format!("users/{user_id}/{}-tryon.jpg", Uuid::new_v4());
    // Generate the thumbnails before the upload below takes the bytes.
    let thumbs = crate::upload::thumbs_from_stored(&processed_bytes);

    state
        .storage
        .upload_file(&key, processed_bytes)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to save generated image: {e}")))?;

    // A preview is displayed like any other image (`?width=480` on the grid), so
    // it gets the same thumbnail objects: without them every view of a preview
    // would re-download and re-decode the 1600px original.
    for (thumb_width, thumb_bytes) in thumbs {
        let thumb_key = crate::upload::thumb_storage_key(&key, thumb_width);
        match state.storage.upload_file(&thumb_key, thumb_bytes).await {
            Err(e) => eprintln!("thumbnail store failed for {thumb_key}: {e}"),
            Ok(_) => {},
        };
    };

    Ok(Json(GeneratePreviewResponse {
        image_path: key,
    }))
}

const MAX_IMAGE_DIMENSION: u32 = 1600;

const JPEG_QUALITY: u8 = 80;
/// Upper bound for the `?width=` resize request to prevent memory DoS.
const MAX_SERVE_WIDTH: u32 = 4096;

fn exif_orientation(data: &[u8]) -> u16 {
    use exif::{In, Reader, Tag, Value};
    let mut reader = std::io::BufReader::new(std::io::Cursor::new(data));
    match Reader::new().read_from_container(&mut reader) {
        Ok(exif) => exif
            .get_field(Tag::Orientation, In(1))
            .and_then(|f| match &f.value {
                Value::Short(v) => v.first().copied(),
                _ => None,
            })
            .unwrap_or(1),
        Err(_) => 1,
    }
}

fn apply_orientation(img: image::RgbaImage, orientation: u16) -> image::RgbaImage {
    use image::imageops;
    match orientation {
        2 => imageops::flip_horizontal(&img),
        3 => imageops::rotate180(&img),
        4 => imageops::flip_vertical(&img),
        5 => imageops::flip_horizontal(&imageops::rotate270(&img)),
        6 => imageops::rotate90(&img),
        7 => imageops::flip_horizontal(&imageops::rotate90(&img)),
        8 => imageops::rotate270(&img),
        _ => img,
    }
}

fn with_jpeg_extension(filename: &str) -> String {
    match filename.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => format!("{stem}.jpg"),
        _ => format!("{filename}.jpg"),
    }
}

fn compress_image(data: &[u8], filename: &str) -> (Vec<u8>, String) {
    let img = match image::load_from_memory(data) {
        Ok(img) => img.to_rgba8(),
        Err(_) => return (data.to_vec(), filename.to_string()),
    };

    let oriented = apply_orientation(img, exif_orientation(data));

    if oriented.pixels().any(|p| p[3] < 255) {
        return (data.to_vec(), filename.to_string());
    }

    let (w, h) = oriented.dimensions();
    let resized = if (w.max(h)) as f64 > MAX_IMAGE_DIMENSION as f64 {
        let scale = MAX_IMAGE_DIMENSION as f64 / (w.max(h)) as f64;
        image::imageops::resize(
            &oriented,
            (w as f64 * scale).round().max(1.0) as u32,
            (h as f64 * scale).round().max(1.0) as u32,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        oriented
    };

    let rgb = image::DynamicImage::ImageRgba8(resized).to_rgb8();
    let mut out = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
    if encoder
        .encode(rgb.as_raw(), rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
        .is_err()
        || out.len() >= data.len()
    {
        return (data.to_vec(), filename.to_string());
    }

    (out, with_jpeg_extension(filename))
}

pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    mut multipart: Multipart,
) -> Result<Json<String>, (StatusCode, String)> {
    let storage = &state.storage;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid multipart body: {e}")))?
    {
        let raw_name = field
            .file_name()
            .map(str::to_owned)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing file name".to_string()))?;

        let data = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("could not read upload: {e}")))?;

        if data.len() > crate::upload::MAX_UPLOAD_BYTES {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "file too large: {} bytes (max {})",
                    data.len(),
                    crate::upload::MAX_UPLOAD_BYTES
                ),
            ));
        }

        let sanitized = crate::upload::sanitize_filename(&raw_name)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

        crate::upload::validate_image_content(&data)
            .map_err(|e| (StatusCode::UNSUPPORTED_MEDIA_TYPE, e.to_string()))?;

        let (data, filename) = compress_image(&data, &sanitized);
        let key_name = if filename == sanitized {
            sanitized
        } else {
            crate::upload::sanitize_filename(&filename)
                .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
        };
        let key = crate::upload::build_storage_key(&user_id, &key_name);

        // Generate the small thumbnails from the processed bytes *before* the
        // upload below moves them (`upload.rs`: `THUMB_WIDTHS`).
        let thumbs = crate::upload::thumbs_from_stored(&data);

        storage
            .upload_file(&key, data)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("storage upload failed: {e}")))?;

        // Also store those thumbnails next to the image, so `/files/*path?width=`
        // can answer with a copy of a small stored object (`serve_file` below)
        // instead of downloading the full-resolution original and decoding it
        // once per `<img>` on every page load. The image itself is already
        // stored, so a thumbnail write is best effort: failing one only means
        // that width falls back to resizing the original.
        for (thumb_width, thumb_bytes) in thumbs {
            let thumb_key = crate::upload::thumb_storage_key(&key, thumb_width);
            match storage.upload_file(&thumb_key, thumb_bytes).await {
                Err(e) => eprintln!("thumbnail store failed for {thumb_key}: {e}"),
                Ok(_) => {},
            };
        };

        return Ok(Json(storage.get_public_url(&key).await));
    }

    Err((StatusCode::BAD_REQUEST, "no file uploaded".to_string()))
}

fn content_type_for(path: &str, stored: &str) -> String {
    let ext = path
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    let inferred = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        _ => stored,
    };
    inferred.to_string()
}

fn resize_image(data: &[u8], width: u32) -> Option<Vec<u8>> {
    let img = image::load_from_memory(data).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    if w <= width {
        return None;
    }
    let scale = width as f64 / w as f64;
    let new_h = (h as f64 * scale).round().max(1.0) as u32;
    let resized =
        image::imageops::resize(&img, width, new_h, image::imageops::FilterType::Lanczos3);
    let rgb = image::DynamicImage::ImageRgba8(resized).to_rgb8();
    let mut out = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
    encoder
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .ok()?;
    Some(out)
}

/// True when a storage object path belongs to `user_id` and contains no
/// traversal. Gates `/files/*path` reads and validates `image_path` values
/// written via the generic table API, so a client can never reference another
/// user's stored file.
fn object_path_within_user(object_path: &str, user_id: &Uuid) -> bool {
    if object_path.contains("..") || object_path.contains('\\') {
        return false;
    }
    object_path.starts_with(&format!("users/{user_id}/"))
}

/// Reduce a stored `image_path` (full storage URL or bare object path) to the
/// bare object path (`users/{uid}/{uuid}-{name}.{ext}`).
fn extract_object_path(image_path: &str) -> &str {
    match image_path.find("users/") {
        Some(idx) => &image_path[idx..],
        None => image_path,
    }
}

/// Validate an `image_path` value supplied in a table write payload. It must be
/// a string that resolves to an object path within the authenticated user's own
/// folder.
fn validate_image_path(value: &JsonValue, user_id: &Uuid) -> Result<(), (StatusCode, String)> {
    if value.is_null() {
        return Ok(());
    }
    let path = value
        .as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "image_path must be a string".into()))?;
    if path.is_empty() {
        return Ok(());
    }
    let object_path = extract_object_path(path);
    if !object_path_within_user(object_path, user_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            "image_path must reference one of your own images".into(),
        ));
    }
    Ok(())
}

/// Cache key for a `/files` response: the object path plus the *effective*
/// width. The width belongs in the key because they are different responses —
/// without it, a cached `?width=480` thumbnail could answer a later request for
/// the untouched original (or the other way round).
fn image_cache_key(path: &str, width: Option<u32>) -> String {
    match width {
        None => format!("{path}?original"),
        Some(w) => format!("{path}?width={w}"),
    }
}

/// Build the response for a served image. Object keys carry a random UUID
/// (`upload.rs`: `build_storage_key`) and a thumbnail keeps the `{uuid}-`
/// prefix of its source object, so no `/files` URL can ever change: the
/// browser may cache a hit for a year and never revalidate it.
fn image_response(image: CachedImage) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, image.content_type)
        .header("cache-control", "public, max-age=31536000, immutable")
        .body(axum::body::Body::from(image.body))
        .unwrap()
}

/// Serve a `?width=` request from the thumbnail objects written next to the
/// image at upload time (`upload.rs`: `THUMB_WIDTHS`, `thumb_storage_key`).
/// The bucket that covers the request is a small stored JPEG, so the exact-bucket
/// case — every grid thumbnail (`?width=480`) — is a byte copy: the original is
/// never downloaded and nothing is decoded. A narrower request (`?width=56` for
/// the picker, `?width=28` for the widget) is resized *from that bucket* rather
/// than from the full-resolution original.
/// `None` means the bucket object is missing, i.e. the image predates this
/// change, so `/files` falls back to `served_from_original`.
async fn served_from_thumb(state: Arc<AppState>, path: &str, width: u32) -> Option<CachedImage> {
    let bucket = match crate::upload::thumbnail_bucket(width) {
        None => return None,
        Some(b) => b,
    };
    let thumb_key = crate::upload::thumb_storage_key(path, bucket);
    let thumb_bytes = match state.storage.download_file(&thumb_key).await {
        Err(_) => return None,
        Ok((bytes, _)) => bytes,
    };

    if bucket == width {
        // Exact match for the widths the UI asks for: the response *is* the
        // stored object, copied straight through with no decode at all.
        return Some(CachedImage { body: thumb_bytes, content_type: "image/jpeg".to_string() });
    }

    // Below the bucket: resize the small object we already hold, so the 4032px
    // original is never decoded to make a 28px thumb.
    match resize_image(&thumb_bytes, width) {
        None => Some(CachedImage { body: thumb_bytes, content_type: "image/jpeg".to_string() }),
        Some(smaller) => Some(CachedImage { body: smaller, content_type: "image/jpeg".to_string() }),
    }
}

/// Fallback when no generated thumbnail exists for `path`: read the original,
/// resize it when asked, and opportunistically store the thumbnails that can be
/// made from the bytes already fetched, so the *next* request for this image
/// takes the byte-copy path above. Images that predate upload-time thumbnails
/// therefore heal themselves one request after the first.
///
/// A thumbnail write that fails is logged but never fails the response: the
/// image itself is already on its way out.
async fn served_from_original(state: Arc<AppState>, path: &str, width: Option<u32>) -> Option<CachedImage> {
    let (data, stored_type) = match state.storage.download_file(&path).await {
        Err(_) => return None,
        Ok(v) => v,
    };

    // Never thumbnail an object that is already a thumbnail: that would nest
    // keys (`...dress.thumb480.thumb96.jpg`) behind a single source image.
    if !crate::upload::is_thumb_key(&path) {
        for (thumb_width, thumb_bytes) in crate::upload::thumbs_from_stored(&data) {
            let thumb_key = crate::upload::thumb_storage_key(&path, thumb_width);
            match state.storage.upload_file(&thumb_key, thumb_bytes).await {
                Err(e) => eprintln!("thumbnail store failed for {thumb_key}: {e}"),
                Ok(_) => {},
            };
        };
    };

    if width.is_none() {
        return Some(CachedImage { body: data, content_type: content_type_for(&path, &stored_type) });
    }

    match width {
        Some(w) if w > 0 => match resize_image(&data, w) {
            Some(resized) => Some(CachedImage { body: resized, content_type: "image/jpeg".to_string() }),
            None => Some(CachedImage { body: data, content_type: content_type_for(&path, &stored_type) }),
        },
        _ => Some(CachedImage { body: data, content_type: content_type_for(&path, &stored_type) }),
    }
}

pub async fn serve_file(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, (StatusCode, String)> {
    // Only serve files that live inside the requesting user's own folder.
    if !object_path_within_user(&path, &user_id) {
        return Err((StatusCode::NOT_FOUND, "file not found".into()));
    }

    let width = params
        .get("width")
        .and_then(|w| w.parse::<u32>().ok())
        .map(|w| w.min(MAX_SERVE_WIDTH))
        .filter(|w| *w > 0);

    let cache_key = image_cache_key(&path, width);

    // A repeat of the exact same URL never touches the network.
    match state.image_cache.get(&cache_key) {
        Some(hit) => return Ok(image_response(hit)),
        None => {},
    };

    // Thumbnails first: for the widths the UI asks for this is a copy of a
    // stored object. Images that predate upload-time thumbnails fall through
    // to the original.
    let from_thumb = match width {
        None => None,
        Some(w) => match crate::upload::is_thumb_key(&path) {
            true => None,
            false => served_from_thumb(state.clone(), &path, w).await,
        },
    };

    let served = match from_thumb {
        Some(image) => Some(image),
        None => served_from_original(state.clone(), &path, width).await,
    };

    match served {
        None => Err((StatusCode::NOT_FOUND, "file not found".into())),
        Some(image) => {
            // Keep a copy so the next request for this exact URL skips the
            // upstream fetch entirely; the response below reuses the bytes we
            // already have.
            state.image_cache.put(&cache_key, image.clone());
            Ok(image_response(image))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "11111111-1111-4111-8111-111111111111";
    const OTHER: &str = "99999999-9999-4999-8999-999999999999";

    fn me() -> Uuid {
        Uuid::parse_str(ME).unwrap()
    }

    #[test]
    fn object_path_within_user_accepts_own_files() {
        assert!(object_path_within_user(
            &format!("users/{ME}/22222222-2222-4222-8222-222222222222-photo.jpg"),
            &me()
        ));
    }

    #[test]
    fn object_path_within_user_rejects_other_users() {
        assert!(!object_path_within_user(
            &format!("users/{OTHER}/22222222-2222-4222-8222-222222222222-photo.jpg"),
            &me()
        ));
    }

    #[test]
    fn object_path_within_user_rejects_traversal() {
        assert!(!object_path_within_user(
            &format!("users/{ME}/../../etc/passwd.png"),
            &me()
        ));
        assert!(!object_path_within_user("users\\evil\\x.png", &me()));
    }

    #[test]
    fn object_path_within_user_rejects_missing_prefix() {
        assert!(!object_path_within_user("public/photo.jpg", &me()));
        assert!(!object_path_within_user("", &me()));
    }

    #[test]
    fn extract_object_path_from_full_url() {
        let url = "https://x.supabase.co/storage/v1/object/public/myfiles/users/11111111-1111-4111-8111-111111111111/abc-photo.jpg";
        assert_eq!(
            extract_object_path(url),
            "users/11111111-1111-4111-8111-111111111111/abc-photo.jpg"
        );
    }

    #[test]
    fn extract_object_path_bare() {
        assert_eq!(
            extract_object_path("users/11111111-1111-4111-8111-111111111111/abc-photo.jpg"),
            "users/11111111-1111-4111-8111-111111111111/abc-photo.jpg"
        );
    }

    #[test]
    fn validate_image_path_accepts_own_and_empty() {
        let u = me();
        assert!(validate_image_path(&JsonValue::Null, &u).is_ok());
        assert!(validate_image_path(&serde_json::json!(""), &u).is_ok());
        assert!(validate_image_path(
            &serde_json::json!("users/11111111-1111-4111-8111-111111111111/abc-photo.jpg"),
            &u
        )
        .is_ok());
        assert!(validate_image_path(
            &serde_json::json!("https://x.supabase.co/storage/v1/object/public/myfiles/users/11111111-1111-4111-8111-111111111111/abc-photo.jpg"),
            &u
        )
        .is_ok());
    }

    #[test]
    fn validate_image_path_rejects_foreign_traversal_and_non_string() {
        let u = me();
        assert!(validate_image_path(
            &serde_json::json!("users/99999999-9999-4999-8999-999999999999/abc-photo.jpg"),
            &u
        )
        .is_err());
        assert!(validate_image_path(
            &serde_json::json!("users/11111111-1111-4111-8111-111111111111/../../etc/passwd.png"),
            &u
        )
        .is_err());
        assert!(validate_image_path(&serde_json::json!(42), &u).is_err());
    }

    #[test]
    fn test_user_settings_blocked_in_generic_crud() {
        assert!(is_generic_crud_table_blocked("user-settings"));
        assert!(!is_generic_crud_table_blocked("outfits"));
        assert!(!is_generic_crud_table_blocked("clothing-items"));
    }

    #[test]
    fn test_ssrf_blocks_metadata_ip() {
        let err = validate_local_qwen_url("http://169.254.169.254/latest/meta-data");
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("metadata"));

        let err2 = validate_local_qwen_url("http://169.254.1.2:8000");
        assert!(err2.is_err());
    }

    #[test]
    fn test_ssrf_blocks_internal_ports() {
        let err_axum = validate_local_qwen_url("http://127.0.0.1:3000");
        assert!(err_axum.is_err());
        assert!(err_axum.unwrap_err().contains("Port 3000 is reserved"));

        let err_pg = validate_local_qwen_url("http://127.0.0.1:5432");
        assert!(err_pg.is_err());
        assert!(err_pg.unwrap_err().contains("Port 5432 is reserved"));
    }

    #[test]
    fn test_ssrf_allows_local_qwen() {
        let ok = validate_local_qwen_url("http://127.0.0.1:8001");
        assert!(ok.is_ok());
        let url = ok.unwrap();
        assert_eq!(url.scheme(), "http");
        assert_eq!(url.port(), Some(8001));

        let ok_lh = validate_local_qwen_url("http://localhost:8001/generate");
        assert!(ok_lh.is_ok());
    }

    #[test]
    fn test_build_qwen_prompt() {
        let items = vec![
            GarmentItemRow {
                id: Uuid::new_v4(),
                name: "Classic Tee".to_string(),
                color_id: Some("Navy".to_string()),
                image_path: Some("users/x/1.jpg".to_string()),
                notes: None,
                category_name: Some("Tops".to_string()),
                subcategory_name: Some("T-Shirts".to_string()),
            },
            GarmentItemRow {
                id: Uuid::new_v4(),
                name: "Slim Jeans".to_string(),
                color_id: Some("Blue".to_string()),
                image_path: Some("users/x/2.jpg".to_string()),
                notes: None,
                category_name: Some("Bottoms".to_string()),
                subcategory_name: Some("Jeans".to_string()),
            },
        ];

        let prompt = build_qwen_prompt(&items);
        assert!(prompt.contains("Picture 1 unchanged"));
        assert!(prompt.contains("Replace the clothes worn by the person in Picture 1 with the garments shown in the following pictures:"));
        assert!(prompt.contains("- Picture 2: Tops (T-Shirts) in Navy, \"Classic Tee\""));
        assert!(prompt.contains("- Picture 3: Bottoms (Jeans) in Blue, \"Slim Jeans\""));
        assert!(prompt.contains("Remove any existing clothing in Picture 1 that conflicts with these garments."));
    }

    #[test]
    fn cache_keys_keep_widths_and_owners_apart() {
        let mine = format!("users/{ME}/22222222-2222-4222-8222-222222222222-photo.jpg");
        let other = format!("users/{OTHER}/22222222-2222-4222-8222-222222222222-photo.jpg");

        // Same object, different responses: the untouched original, the 480 px
        // grid thumbnail and the 96 px widget thumbnail must never answer for
        // each other.
        assert_ne!(image_cache_key(&mine, None), image_cache_key(&mine, Some(480)));
        assert_ne!(image_cache_key(&mine, Some(480)), image_cache_key(&mine, Some(96)));
        // Two users whose paths otherwise look alike never share a slot.
        assert_ne!(image_cache_key(&mine, Some(480)), image_cache_key(&other, Some(480)));
        // A thumbnail object is its own entry, not the source image's.
        assert_ne!(
            image_cache_key(&mine, Some(480)),
            image_cache_key(&crate::upload::thumb_storage_key(&mine, 480), Some(480)),
        );
    }

    #[test]
    fn thumbnail_keys_stay_inside_the_owners_folder() {
        // `/files` derives thumbnail keys itself and then serves them through
        // the same ownership check as any other path, so a derived key has to
        // pass that check for its own owner and be refused for another user.
        let thumb =
            crate::upload::thumb_storage_key(&format!("users/{ME}/22222222-2222-4222-8222-222222222222-photo.jpg"), 480);
        assert!(object_path_within_user(&thumb, &me()));

        let intruder = Uuid::parse_str(OTHER).unwrap();
        assert!(!object_path_within_user(&thumb, &intruder));
    }
}

