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
    // Outfit deletion must go through DELETE /outfits/:id so wear counts stay in sync.
    if table == "outfits" {
        return Err((
            StatusCode::METHOD_NOT_ALLOWED,
            "use DELETE /outfits/:id to delete outfits".into(),
        ));
    }
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

    sqlx::query(
        "UPDATE \"clothing-items\" ci \
         SET wear_count = COALESCE(ci.wear_count, 0) + 1, updated_at = now() \
         FROM (SELECT DISTINCT oi.clothing_item_id FROM \"outfit-items\" oi WHERE oi.outfit_id = $1) s \
         WHERE ci.id = s.clothing_item_id AND ci.user_id = $2",
    )
    .bind(&outfit_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("could not update wear counts: {e}")))?;

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
         FROM (SELECT DISTINCT oi.clothing_item_id FROM \"outfit-items\" oi WHERE oi.outfit_id = $1) s \
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

        storage
            .upload_file(&key, data)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("storage upload failed: {e}")))?;

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

pub async fn serve_file(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, (StatusCode, String)> {
    let storage = &state.storage;

    // Only serve files that live inside the requesting user's own folder.
    if !object_path_within_user(&path, &user_id) {
        return Err((StatusCode::NOT_FOUND, "file not found".into()));
    }

    let (data, content_type) = match storage.download_file(&path).await {
        Ok(v) => v,
        Err(_) => return Err((StatusCode::NOT_FOUND, "file not found".into())),
    };

    let width = params
        .get("width")
        .and_then(|w| w.parse::<u32>().ok())
        .map(|w| w.min(MAX_SERVE_WIDTH));

    let (body, content_type) = match width {
        Some(w) if w > 0 => match resize_image(&data, w) {
            Some(resized) => (resized, "image/jpeg".to_string()),
            None => (data, content_type_for(&path, &content_type)),
        },
        _ => (data, content_type_for(&path, &content_type)),
    };

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .body(axum::body::Body::from(body))
        .unwrap())
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
}
