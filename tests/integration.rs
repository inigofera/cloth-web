//! End-to-end integration tests: a real Postgres (the full migration chain
//! applied to a clean schema) plus in-process mocks of the Supabase Storage
//! API, the Gemini REST endpoint, and the local Qwen worker. The app under
//! test is built with the same `build_router` the binary uses, so these
//! tests exercise exactly what ships.
//!
//! Skipped gracefully when no test database is reachable: set
//! `CLOTHWEB_TEST_DATABASE_URL`, or run Postgres at 127.0.0.1:5432 with a
//! `cloth_test` database (CI sets the variable explicitly via a service
//! container).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use serde_json::Value;
use sqlx::postgres::{PgPoolOptions, Postgres};
use sqlx::Pool;
use tower::util::ServiceExt;
use uuid::Uuid;

use backend::{
    build_router, cache::ImageCache, db::SchemaCache, ratelimit::RateLimiter,
    storage::SupabaseStorage, AppState,
};

const TEST_SECRET: &str = "integration-test-jwt-secret";
const TEST_ISSUER: &str = "https://integration.supabase.test";

fn db_candidates() -> Vec<String> {
    let mut urls = Vec::new();
    if let Ok(u) = std::env::var("CLOTHWEB_TEST_DATABASE_URL") {
        if !u.trim().is_empty() {
            urls.push(u);
        }
    }
    urls.push("postgres://postgres@127.0.0.1:5432/cloth_test".to_string());
    urls.push("postgres://postgres:postgres@127.0.0.1:5432/cloth_test".to_string());
    urls
}

async fn connect(url: &str) -> Option<Pool<Postgres>> {
    // The whole binary shares one pool: every test's direct queries and every
    // handler the tests drive draw from it at once. Size it far above the
    // worst-case concurrent demand (18 tests x a few in-flight queries) so
    // no test can starve the others, and give the acquire a generous window
    // for a cold CI database.
    PgPoolOptions::new()
        .max_connections(64)
        .min_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(url)
        .await
        .ok()
}

// ---------------------------------------------------------------------------
// In-process Supabase Storage mock: an axum server that answers the REST
// paths `SupabaseStorage` calls (PUT/GET /storage/v1/object/{bucket}/{key}).
// Objects live in a shared in-memory map so tests can compare served bytes
// against what was stored.
// ---------------------------------------------------------------------------

const BUCKET: &str = "test-bucket";

/// An uploaded object: its bytes plus the content type the upload carried.
type StoredObject = (Vec<u8>, String);

#[derive(Clone)]
struct MockStore {
    inner: Arc<Mutex<HashMap<String, StoredObject>>>,
}

impl MockStore {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// The mock stores objects under `{bucket}/{object_key}` (the path after
    /// `/storage/v1/object/`), so look up by the bare object key.
    fn object(&self, key: &str) -> Option<StoredObject> {
        self.inner
            .lock()
            .unwrap()
            .get(&format!("{BUCKET}/{key}"))
            .cloned()
    }
}

async fn storage_put(
    Path(key): Path<String>,
    State(store): State<MockStore>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    store
        .inner
        .lock()
        .unwrap()
        .insert(key, (body.to_vec(), content_type));
    StatusCode::OK
}

async fn storage_get(
    Path(key): Path<String>,
    State(store): State<MockStore>,
) -> Result<Response, StatusCode> {
    let entry = store
        .inner
        .lock()
        .unwrap()
        .get(&key)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, &entry.1)
        .body(axum::body::Body::from(entry.0))
        .unwrap())
}

async fn start_storage_mock(store: MockStore) -> String {
    let router = Router::new()
        .route(
            "/storage/v1/object/*path",
            get(storage_get).put(storage_put),
        )
        .route("/storage/v1/object/public/*path", get(storage_get))
        .with_state(store);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

// ---------------------------------------------------------------------------
// In-process provider mocks for the try-on flow: one answers the Gemini
// REST endpoint (`{gemini_api_base}/models/gemini-2.5-flash-image:generateContent`),
// the other the local Qwen worker (`{local_qwen_url}/generate` + `/health`).
// Both record every request so scenarios can assert provider dispatch and
// request content. Counters are reset per scenario (scenarios run serially).
// ---------------------------------------------------------------------------

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

#[derive(Clone, Default)]
struct GeminiLog {
    /// One entry per request: the `x-goog-api-key` header (empty if absent).
    keys: Vec<String>,
    /// One entry per request: the raw request body, for content assertions.
    bodies: Vec<String>,
}

#[derive(Clone)]
struct GeminiMock {
    inner: Arc<Mutex<GeminiLog>>,
}

impl GeminiMock {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(GeminiLog::default())),
        }
    }

    fn reset(&self) {
        self.inner.lock().unwrap().keys.clear();
        self.inner.lock().unwrap().bodies.clear();
    }

    /// (request count, last key, last body)
    fn snapshot(&self) -> (usize, Option<String>, Option<String>) {
        let log = self.inner.lock().unwrap();
        (
            log.keys.len(),
            log.keys.last().cloned(),
            log.bodies.last().cloned(),
        )
    }
}

async fn gemini_generate(
    Path(_rest): Path<String>,
    State(mock): State<GeminiMock>,
    headers: HeaderMap,
    body: Bytes,
) -> axum::Json<Value> {
    let key = headers
        .get("x-goog-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let mut log = mock.inner.lock().unwrap();
    log.keys.push(key);
    log.bodies.push(String::from_utf8_lossy(&body).to_string());
    drop(log);

    let b64 = B64.encode(wide_jpeg());
    axum::Json(serde_json::json!({
        "candidates": [{
            "finishReason": "STOP",
            "content": {
                "parts": [{ "inlineData": { "mimeType": "image/jpeg", "data": b64 } }]
            }
        }]
    }))
}

async fn start_gemini_mock(mock: GeminiMock) -> String {
    let router = Router::new()
        .route("/v1beta/models/*rest", post(gemini_generate))
        .with_state(mock);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

#[derive(Clone)]
struct QwenMock {
    inner: Arc<Mutex<Vec<Value>>>,
}

impl QwenMock {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn reset(&self) {
        self.inner.lock().unwrap().clear();
    }

    fn requests(&self) -> Vec<Value> {
        self.inner.lock().unwrap().clone()
    }
}

async fn qwen_generate(State(mock): State<QwenMock>, body: Bytes) -> axum::Json<Value> {
    let parsed: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    mock.inner.lock().unwrap().push(parsed);
    let b64 = B64.encode(wide_jpeg());
    axum::Json(serde_json::json!({ "image_base64": b64 }))
}

async fn qwen_health() -> axum::Json<Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "model": "qwen-image-2.1-turbo-Q6_K_XL.gguf",
        "ready": true
    }))
}

async fn start_qwen_mock(mock: QwenMock) -> String {
    let router = Router::new()
        .route("/generate", post(qwen_generate))
        .route("/health", get(qwen_health))
        .with_state(mock);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

// ---------------------------------------------------------------------------
// Shared app: one database reset + one migration pass + one mock, shared by
// every test in this binary.
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct TestApp {
    router: Router,
    pool: Pool<Postgres>,
    store: MockStore,
    gemini: GeminiMock,
    qwen: QwenMock,
    /// The Qwen worker base URL each scenario points its user settings at.
    qwen_url: String,
}

static APP: tokio::sync::OnceCell<TestApp> = tokio::sync::OnceCell::const_new();

async fn app() -> Option<TestApp> {
    let built = APP
        .get_or_init(|| async {
            let mut pool = None;
            for url in db_candidates() {
                if let Some(p) = connect(&url).await {
                    pool = Some(p);
                    break;
                }
            }
            let pool = pool.expect("no test database reachable");

            // Reset: the suite owns this database. Emulate the Supabase `auth`
            // schema (users + auth.uid()) that the migrations reference.
            sqlx::raw_sql(
                "DROP SCHEMA IF EXISTS public CASCADE;
             CREATE SCHEMA public;
             DROP SCHEMA IF EXISTS auth CASCADE;
             CREATE SCHEMA auth;
             CREATE TABLE auth.users (id uuid PRIMARY KEY);
             CREATE FUNCTION auth.uid() RETURNS uuid
             LANGUAGE sql STABLE AS $$
                 SELECT nullif(current_setting('request.jwt.claim.sub', true), '')::uuid
             $$;",
            )
            .execute(&pool)
            .await
            .unwrap();

            // Apply the real migration chain, in filename order: this is the
            // proof that a fresh project bootstraps from the repo alone.
            let mut files: Vec<_> = std::fs::read_dir("supabase/migrations")
                .unwrap()
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .collect();
            files.sort();
            for path in &files {
                let sql = std::fs::read_to_string(path).unwrap();
                // Checked-in migration files: trusted content, not request data.
                sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
                    .execute(&pool)
                    .await
                    .unwrap();
            }

            let store = MockStore::new();
            let base_url = start_storage_mock(store.clone()).await;

            let gemini = GeminiMock::new();
            let gemini_url = start_gemini_mock(gemini.clone()).await;
            let qwen = QwenMock::new();
            let qwen_url = start_qwen_mock(qwen.clone()).await;

            let schema = Arc::new(SchemaCache::new());
            schema.refresh(&pool).await;

            let state = Arc::new(AppState {
                db: pool.clone(),
                storage: SupabaseStorage::with(base_url, "test-service-key".into(), BUCKET.into()),
                image_cache: Arc::new(ImageCache::new(64, 4 * 1024 * 1024)),
                rate: Arc::new(RateLimiter::new()),
                schema,
                jwt_secret: TEST_SECRET.to_string(),
                jwt_issuer: TEST_ISSUER.to_string(),
                encryption_key: [7u8; 32],
                // The handler appends `/models/...` to the base; production's
                // base ends in `/v1beta`, so the test mirror it.
                gemini_api_base: format!("{gemini_url}/v1beta"),
            });

            TestApp {
                router: build_router(state),
                pool,
                store,
                gemini,
                qwen,
                qwen_url,
            }
        })
        .await;
    Some(built.clone())
}

// ---------------------------------------------------------------------------
// Request/response helpers
// ---------------------------------------------------------------------------

fn make_request(
    method: &str,
    uri: &str,
    auth: Option<&str>,
    body: Vec<u8>,
    content_type: Option<&str>,
) -> Request<axum::body::Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(a) = auth {
        builder = builder.header("authorization", format!("Bearer {a}"));
    }
    if let Some(ct) = content_type {
        builder = builder.header("content-type", ct);
    }
    builder.body(axum::body::Body::from(body)).unwrap()
}

async fn call(app: &TestApp, req: Request<axum::body::Body>) -> Response {
    app.router.clone().oneshot(req).await.unwrap()
}

async fn body_bytes(resp: Response) -> Vec<u8> {
    axum::body::to_bytes(resp.into_body(), 100 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec()
}

async fn body_json(resp: Response) -> Value {
    let bytes = body_bytes(resp).await;
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

fn jwt(sub: &Uuid, role: &str, issuer: &str, exp_offset_secs: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let claims = serde_json::json!({
        "sub": sub.to_string(),
        "aud": "authenticated",
        "role": role,
        "iss": issuer,
        "exp": now + exp_offset_secs,
    });
    jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(TEST_SECRET.as_bytes()),
    )
    .unwrap()
}

async fn fresh_user(pool: &Pool<Postgres>) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO auth.users (id) VALUES ($1)")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    id
}

/// A tiny valid JPEG for upload tests, built with the `image` crate.
fn tiny_jpeg() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(16, 16, image::Rgb([200, 30, 90]));
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut out)
        .encode(img.as_raw(), 16, 16, image::ExtendedColorType::Rgb8)
        .unwrap();
    out
}

/// An 800x800 JPEG, wide enough that `thumbs_from_stored` emits both
/// thumbnails (thumbnails never upscale, so the tiny fixture yields none).
/// This mirrors what a real generation provider returns, not a test fixture.
fn wide_jpeg() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(800, 800, image::Rgb([40, 120, 200]));
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut out)
        .encode(img.as_raw(), 800, 800, image::ExtendedColorType::Rgb8)
        .unwrap();
    out
}

fn multipart(filename: &str, content_type: &str, data: &[u8]) -> (Vec<u8>, String) {
    let boundary = "testboundary42";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (body, format!("multipart/form-data; boundary={boundary}"))
}

// ---------------------------------------------------------------------------
// Health, security headers, authentication
// ---------------------------------------------------------------------------

async fn healthz_reports_ok_and_sets_security_headers() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };

    let resp = call(&app, make_request("GET", "/healthz", None, vec![], None)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body, serde_json::json!({ "status": "ok" }));
}

async fn every_response_carries_the_hygiene_headers() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };

    // A 401 response passes through the middleware the same way any other
    // response does.
    let resp = call(&app, make_request("GET", "/tables", None, vec![], None)).await;
    assert_eq!(
        resp.headers()
            .get("x-content-type-options")
            .and_then(|v| v.to_str().ok()),
        Some("nosniff")
    );
    assert_eq!(
        resp.headers()
            .get("referrer-policy")
            .and_then(|v| v.to_str().ok()),
        Some("no-referrer")
    );

    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);
    let resp = call(
        &app,
        make_request("GET", "/healthz", Some(&token), vec![], None),
    )
    .await;
    assert_eq!(
        resp.headers()
            .get("x-content-type-options")
            .and_then(|v| v.to_str().ok()),
        Some("nosniff")
    );
}

async fn rejects_unauthenticated_and_invalid_tokens() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;

    // No header at all.
    let resp = call(&app, make_request("GET", "/tables", None, vec![], None)).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Garbage bearer.
    let resp = call(
        &app,
        make_request("GET", "/tables", Some("Bearer not-a-jwt"), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Signed by the right secret but a different project (wrong `iss`).
    let wrong_issuer = jwt(&me, "authenticated", "https://other.supabase.co", 3600);
    let resp = call(
        &app,
        make_request("GET", "/tables", Some(&wrong_issuer), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Service-role tokens must never authenticate against the user API.
    let service = jwt(&me, "service_role", TEST_ISSUER, 3600);
    let resp = call(
        &app,
        make_request("GET", "/tables", Some(&service), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Expired: well past the 60 s clock-skew leeway the verifier allows.
    let expired = jwt(&me, "authenticated", TEST_ISSUER, -3600);
    let resp = call(
        &app,
        make_request("GET", "/tables", Some(&expired), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // A valid token still works: 200 with the schema.
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);
    let resp = call(
        &app,
        make_request("GET", "/tables", Some(&token), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// /tables listing
// ---------------------------------------------------------------------------

async fn tables_listing_shares_the_schema_and_hides_blocked_tables() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);

    let resp = call(
        &app,
        make_request("GET", "/tables", Some(&token), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let rows: Vec<Value> =
        serde_json::from_str(&String::from_utf8(body_bytes(resp).await).unwrap()).unwrap();
    // One row per column; the tables are the distinct table names.
    let mut tables: Vec<String> = rows
        .iter()
        .filter_map(|r| r["table_name"].as_str().map(String::from))
        .collect();
    tables.sort();
    tables.dedup();

    // Every public table except the two the generic API must never expose.
    assert_eq!(
        tables,
        vec![
            "brands",
            "clothing-categories",
            "clothing-items",
            "clothing-subcategories",
            "colors",
            "outfit-items",
            "outfits",
        ]
    );

    // Spot-check that a column shape came through the schema read.
    assert!(rows.iter().any(|r| r["table_name"] == "clothing-items"
        && r["column_name"] == "wear_count"
        && r["data_type"] == "integer"));
}

// ---------------------------------------------------------------------------
// Generic CRUD against a user-scoped table
// ---------------------------------------------------------------------------

async fn generic_crud_scopes_everything_to_the_caller() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let other = fresh_user(&app.pool).await;
    let me_tok = jwt(&me, "authenticated", TEST_ISSUER, 3600);
    let other_tok = jwt(&other, "authenticated", TEST_ISSUER, 3600);

    // Insert: the server returns the created row with an id and timestamps.
    let payload = serde_json::to_vec(&serde_json::json!({ "name": "Blue Shirt", "purchase_price": 19.99, "origin": "Bought New" })).unwrap();
    let resp = call(
        &app,
        make_request(
            "POST",
            "/db/clothing-items",
            Some(&me_tok),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let row = body_json(resp).await;
    let item_id = row["id"].as_str().unwrap().to_string();
    assert_eq!(row["user_id"].as_str().unwrap(), me.to_string());
    assert!(row["created_at"].is_string());

    // List: the caller sees exactly their own row, and the total is exact.
    let resp = call(
        &app,
        make_request(
            "GET",
            "/db/clothing-items?limit=10&offset=0",
            Some(&me_tok),
            vec![],
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("X-Total-Count")
            .and_then(|v| v.to_str().ok()),
        Some("1")
    );
    let rows: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"].as_str().unwrap(), "Blue Shirt");

    // The other user sees an empty table, not mine.
    let resp = call(
        &app,
        make_request("GET", "/db/clothing-items", Some(&other_tok), vec![], None),
    )
    .await;
    assert_eq!(
        resp.headers()
            .get("X-Total-Count")
            .and_then(|v| v.to_str().ok()),
        Some("0")
    );

    // Patch: the owner updates the row; the other user's patch matches nothing.
    let payload = serde_json::to_vec(&serde_json::json!({ "notes": "favorite" })).unwrap();
    let resp = call(
        &app,
        make_request(
            "PATCH",
            &format!("/db/clothing-items/{item_id}"),
            Some(&me_tok),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let row = body_json(resp).await;
    assert_eq!(row["notes"].as_str().unwrap(), "favorite");
    assert_ne!(row["updated_at"].as_str().unwrap(), "");

    let payload = serde_json::to_vec(&serde_json::json!({ "notes": "hacked" })).unwrap();
    let resp = call(
        &app,
        make_request(
            "PATCH",
            &format!("/db/clothing-items/{item_id}"),
            Some(&other_tok),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_json(resp).await.is_null());

    // Delete: the owner deletes; the other user's delete matches nothing.
    let resp = call(
        &app,
        make_request(
            "DELETE",
            &format!("/db/clothing-items/{item_id}"),
            Some(&other_tok),
            vec![],
            None,
        ),
    )
    .await;
    assert!(body_json(resp).await.is_null());
    let resp = call(
        &app,
        make_request(
            "DELETE",
            &format!("/db/clothing-items/{item_id}"),
            Some(&me_tok),
            vec![],
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let row = body_json(resp).await;
    assert_eq!(row["id"].as_str().unwrap(), item_id);
    // A second delete finds nothing.
    let resp = call(
        &app,
        make_request(
            "DELETE",
            &format!("/db/clothing-items/{item_id}"),
            Some(&me_tok),
            vec![],
            None,
        ),
    )
    .await;
    assert!(body_json(resp).await.is_null());
}

async fn insert_forces_caller_ownership_and_generates_the_id() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let other = fresh_user(&app.pool).await;
    let me_tok = jwt(&me, "authenticated", TEST_ISSUER, 3600);

    // A body naming someone else's user_id is overridden to the caller.
    let payload =
        serde_json::to_vec(&serde_json::json!({ "name": "Owns Me", "user_id": other.to_string() }))
            .unwrap();
    let resp = call(
        &app,
        make_request(
            "POST",
            "/db/clothing-items",
            Some(&me_tok),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let row = body_json(resp).await;
    assert_eq!(row["user_id"].as_str().unwrap(), me.to_string());

    // A body with no id gets one generated by the server.
    let payload = serde_json::to_vec(&serde_json::json!({ "name": "Has An Id" })).unwrap();
    let resp = call(
        &app,
        make_request(
            "POST",
            "/db/clothing-items",
            Some(&me_tok),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    let row = body_json(resp).await;
    assert!(Uuid::parse_str(row["id"].as_str().unwrap()).is_ok());

    // Constraint violations from the DB map to a 400, not a panic.
    let payload =
        serde_json::to_vec(&serde_json::json!({ "name": "Bad", "origin": "Alien Trade" })).unwrap();
    let resp = call(
        &app,
        make_request(
            "POST",
            "/db/clothing-items",
            Some(&me_tok),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

async fn blocked_tables_reject_generic_access() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);

    for table in ["user-settings", "discord-link-attempts"] {
        let resp = call(
            &app,
            make_request("GET", &format!("/db/{table}"), Some(&token), vec![], None),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "GET /db/{table}");

        let payload = serde_json::to_vec(&serde_json::json!({ "name": "x" })).unwrap();
        let resp = call(
            &app,
            make_request(
                "POST",
                &format!("/db/{table}"),
                Some(&token),
                payload,
                Some("application/json"),
            ),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "POST /db/{table}");

        let resp = call(
            &app,
            make_request(
                "DELETE",
                &format!("/db/{table}/whatever"),
                Some(&token),
                vec![],
                None,
            ),
        )
        .await;
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "DELETE /db/{table}/id"
        );
    }
}

async fn shared_reference_tables_are_read_only() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);

    // Reading works: colors are shared-or-owned now, so a fresh user sees the
    // shared rows (none here) plus their own.
    let resp = call(
        &app,
        make_request("GET", "/db/colors", Some(&token), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // But writing through the generic API is refused: the dedicated endpoints
    // own color writes (scoping + slug validation). The same applies to the
    // other reference tables and to outfits.
    let payload =
        serde_json::to_vec(&serde_json::json!({ "id": "hacked", "hex_value": "#ff0000" })).unwrap();
    for (method, path) in [
        ("POST", "/db/colors"),
        ("POST", "/db/clothing-categories"),
        ("POST", "/db/brands"),
        ("PATCH", "/db/outfits/some-id"),
        ("DELETE", "/db/outfits/some-id"),
    ] {
        let resp = call(
            &app,
            make_request(
                method,
                path,
                Some(&token),
                payload.clone(),
                Some("application/json"),
            ),
        )
        .await;
        assert_eq!(
            resp.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "{method} {path} must be refused for dedicated-write tables"
        );
    }
}

// ---------------------------------------------------------------------------
// Outfit lifecycle: item ids, wear counts, transactional delete
// ---------------------------------------------------------------------------

async fn create_item(app: &TestApp, token: &str, name: &str) -> String {
    let payload = serde_json::to_vec(&serde_json::json!({ "name": name })).unwrap();
    let resp = call(
        app,
        make_request(
            "POST",
            "/db/clothing-items",
            Some(token),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    body_json(resp).await["id"].as_str().unwrap().to_string()
}

async fn outfit_lifecycle_keeps_wear_counts_in_sync() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);

    // Two items, passed to the outfit in *unsorted* name order.
    let red = create_item(&app, &token, "Red Pants").await;
    let blue = create_item(&app, &token, "Blue Shirt").await;

    let payload = serde_json::to_vec(&serde_json::json!({
        "date": "2026-11-19",
        "notes": "errand day",
        "item_ids": [red, blue],
    }))
    .unwrap();
    let resp = call(
        &app,
        make_request(
            "POST",
            "/outfits",
            Some(&token),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let outfit = body_json(resp).await;
    let outfit_id = outfit["id"].as_str().unwrap().to_string();
    // The page carries the ids ordered by item name, not insertion order.
    assert_eq!(
        outfit["item_ids"].as_array().unwrap(),
        &vec![serde_json::json!(blue), serde_json::json!(red)]
    );
    assert_eq!(outfit["date"].as_str().unwrap(), "2026-11-19");

    // The list endpoint agrees, with an exact total.
    let resp = call(
        &app,
        make_request(
            "GET",
            "/outfits?limit=10&offset=0",
            Some(&token),
            vec![],
            None,
        ),
    )
    .await;
    assert_eq!(
        resp.headers()
            .get("X-Total-Count")
            .and_then(|v| v.to_str().ok()),
        Some("1")
    );
    let rows: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert_eq!(rows[0]["id"].as_str().unwrap(), outfit_id);

    // Wearing both items once bumped both wear counts.
    let resp = call(
        &app,
        make_request("GET", "/db/clothing-items", Some(&token), vec![], None),
    )
    .await;
    let rows: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert!(rows.iter().all(|r| r["wear_count"].as_i64() == Some(1)));

    // Deleting the outfit drops the links and the wear counts, atomically.
    let resp = call(
        &app,
        make_request(
            "DELETE",
            &format!("/outfits/{outfit_id}"),
            Some(&token),
            vec![],
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["id"].as_str().unwrap(), outfit_id);

    let resp = call(
        &app,
        make_request("GET", "/db/clothing-items", Some(&token), vec![], None),
    )
    .await;
    let rows: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert!(rows.iter().all(|r| r["wear_count"].as_i64() == Some(0)));

    // And the generic API refuses outfit deletion outright (wear-count sync).
    let resp = call(
        &app,
        make_request(
            "DELETE",
            &format!("/db/outfits/{outfit_id}"),
            Some(&token),
            vec![],
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    // Second outfit delete: 404, not panic.
    let resp = call(
        &app,
        make_request(
            "DELETE",
            &format!("/outfits/{outfit_id}"),
            Some(&token),
            vec![],
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

async fn outfit_validation_rejects_foreign_and_malformed_items() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let other = fresh_user(&app.pool).await;
    let me_tok = jwt(&me, "authenticated", TEST_ISSUER, 3600);
    let other_tok = jwt(&other, "authenticated", TEST_ISSUER, 3600);

    // Not a uuid at all.
    let payload = serde_json::to_vec(
        &serde_json::json!({ "date": "2026-11-19", "item_ids": ["not-a-uuid"] }),
    )
    .unwrap();
    let resp = call(
        &app,
        make_request(
            "POST",
            "/outfits",
            Some(&me_tok),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // A real item, but owned by someone else: 400, and nothing is written.
    let theirs = create_item(&app, &other_tok, "Foreign Tee").await;
    let payload =
        serde_json::to_vec(&serde_json::json!({ "date": "2026-11-19", "item_ids": [theirs] }))
            .unwrap();
    let resp = call(
        &app,
        make_request(
            "POST",
            "/outfits",
            Some(&me_tok),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = call(
        &app,
        make_request("GET", "/outfits", Some(&me_tok), vec![], None),
    )
    .await;
    assert_eq!(
        resp.headers()
            .get("X-Total-Count")
            .and_then(|v| v.to_str().ok()),
        Some("0")
    );

    // The hard cap is 200 items; the length check fires before any DB access,
    // so the ids need not exist. (Duplicate ids are dropped, so this must be
    // 201 *distinct* ids to trip the cap.)
    let ids: Vec<Value> = (0..201)
        .map(|_| serde_json::json!(Uuid::new_v4()))
        .collect();
    let payload =
        serde_json::to_vec(&serde_json::json!({ "date": "2026-11-19", "item_ids": ids })).unwrap();
    let resp = call(
        &app,
        make_request(
            "POST",
            "/outfits",
            Some(&me_tok),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Settings: encrypted API keys, validation, Discord link code
// ---------------------------------------------------------------------------

async fn settings_roundtrip_encrypts_api_keys() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);

    // Defaults for a fresh user.
    let resp = call(
        &app,
        make_request("GET", "/settings", Some(&token), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let s = body_json(resp).await;
    assert_eq!(s["tryon_provider"].as_str().unwrap(), "gemini");
    assert!(!s["has_api_key"].as_bool().unwrap());
    assert!(s["api_key_hint"].is_null());
    assert_eq!(
        s["local_qwen_url"].as_str().unwrap(),
        "http://127.0.0.1:8001"
    );

    // Storing a key: the response reveals only a hint, the DB holds ciphertext.
    let key = "sk-test-abc-123456";
    let payload = serde_json::to_vec(&serde_json::json!({ "api_key": key })).unwrap();
    let resp = call(
        &app,
        make_request(
            "PUT",
            "/settings",
            Some(&token),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let s = body_json(resp).await;
    assert!(s["has_api_key"].as_bool().unwrap());
    assert_eq!(s["api_key_hint"].as_str().unwrap(), "...3456");

    let enc: (Option<Vec<u8>>, Option<Vec<u8>>) = sqlx::query_as(
        "SELECT encrypted_api_key, api_key_nonce FROM public.\"user-settings\" WHERE user_id = $1",
    )
    .bind(me)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(!enc.0.as_ref().unwrap().is_empty());
    assert!(!enc.1.as_ref().unwrap().is_empty());
    assert_ne!(
        &enc.0.unwrap(),
        key.as_bytes(),
        "API key must not be stored in plaintext"
    );

    // Provider validation.
    let payload = serde_json::to_vec(&serde_json::json!({ "tryon_provider": "nope" })).unwrap();
    let resp = call(
        &app,
        make_request(
            "PUT",
            "/settings",
            Some(&token),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // An empty string clears the key.
    let payload = serde_json::to_vec(&serde_json::json!({ "api_key": "" })).unwrap();
    let resp = call(
        &app,
        make_request(
            "PUT",
            "/settings",
            Some(&token),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!body_json(resp).await["has_api_key"].as_bool().unwrap());
}

async fn discord_link_code_is_stored_with_a_15_minute_expiry() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);

    let resp = call(
        &app,
        make_request("POST", "/settings/discord-code", Some(&token), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let s = body_json(resp).await;
    let code = s["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 6, "code must be 6 digits");
    assert!(code.chars().all(|c| c.is_ascii_digit()));
    assert_eq!(s["expires_in_seconds"].as_u64().unwrap(), 900);

    // The DB agrees, and the expiry is ~15 minutes out (checked in SQL).
    let (stored_code, expiry_sane): (String, bool) = sqlx::query_as(
        "SELECT discord_link_code,
                 discord_link_expires_at > now() AND discord_link_expires_at < now() + interval '16 minutes'
         FROM public.\"user-settings\" WHERE user_id = $1",
    )
    .bind(me)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(stored_code, code);
    assert!(expiry_sane);
}

async fn unlink_discord_clears_the_pairing() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);

    // Pair the account directly, then unlink through the API.
    sqlx::query(
        r#"INSERT INTO public."user-settings" (user_id, discord_user_id, updated_at)
           VALUES ($1, 'discord-12345', now())"#,
    )
    .bind(me)
    .execute(&app.pool)
    .await
    .unwrap();

    let resp = call(
        &app,
        make_request(
            "POST",
            "/settings/discord-unlink",
            Some(&token),
            vec![],
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_json(resp).await["discord_user_id"].is_null());
}

// ---------------------------------------------------------------------------
// Uploads and image serving: storage round-trip, ownership, signed tokens
// ---------------------------------------------------------------------------

async fn upload_image(app: &TestApp, token: &str) -> String {
    let (body, ct) = multipart("photo.jpg", "image/jpeg", &tiny_jpeg());
    let resp = call(
        app,
        make_request("POST", "/upload", Some(token), body, Some(&ct)),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "upload failed: {}",
        String::from_utf8(body_bytes(resp).await).unwrap_or_default()
    );
    // The response is the public URL itself; the object key is its tail.
    let url = body_json(resp).await.as_str().unwrap().to_string();
    let idx = url.find("users/").unwrap();
    url[idx..].to_string()
}

async fn upload_and_serve_respect_auth_and_ownership() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let other = fresh_user(&app.pool).await;
    let me_tok = jwt(&me, "authenticated", TEST_ISSUER, 3600);
    let other_tok = jwt(&other, "authenticated", TEST_ISSUER, 3600);

    // Upload lands inside the caller's own folder.
    let key = upload_image(&app, &me_tok).await;
    assert!(
        key.starts_with(&format!("users/{me}/")),
        "object key: {key}"
    );

    // Unauthenticated serving is refused.
    let resp = call(
        &app,
        make_request("GET", &format!("/files/{key}"), None, vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // The owner gets their exact bytes back, with the immutable cache headers.
    let resp = call(
        &app,
        make_request("GET", &format!("/files/{key}"), Some(&me_tok), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("image/jpeg")
    );
    assert!(resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .contains("immutable"));
    let served = body_bytes(resp).await;
    let stored = app.store.object(&key).unwrap();
    assert_eq!(served, stored.0, "served bytes must equal stored bytes");

    // Another user's JWT cannot read my file: 404, not a cross-read.
    let resp = call(
        &app,
        make_request(
            "GET",
            &format!("/files/{key}"),
            Some(&other_tok),
            vec![],
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Path traversal inside the wildcard is refused. Authenticated, so the
    // request reaches the handler's own-folder check (unauthenticated it is a
    // 401 from the auth extractor before any path logic runs).
    let resp = call(
        &app,
        make_request(
            "GET",
            &format!("/files/users/{me}/../../etc/passwd"),
            Some(&me_tok),
            vec![],
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

async fn signed_file_tokens_gate_image_serving() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let other = fresh_user(&app.pool).await;
    let me_tok = jwt(&me, "authenticated", TEST_ISSUER, 3600);
    let other_tok = jwt(&other, "authenticated", TEST_ISSUER, 3600);

    let key = upload_image(&app, &me_tok).await;

    // /files/sign itself needs the Bearer JWT...
    let resp = call(&app, make_request("GET", "/files/sign", None, vec![], None)).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // ...and returns a short-lived token plus its expiry.
    let resp = call(
        &app,
        make_request("GET", "/files/sign", Some(&me_tok), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let s = body_json(resp).await;
    let token = s["token"].as_str().unwrap().to_string();
    let exp = s["expires_at"].as_i64().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    assert!(
        exp > now + 500 && exp <= now + 610,
        "token TTL should be ~10 minutes"
    );

    // The token works in a query string, exactly like an <img> src would use it.
    let uri = format!("/files/{key}?token={token}&exp={exp}");
    let resp = call(&app, make_request("GET", &uri, None, vec![], None)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let served = body_bytes(resp).await;
    assert_eq!(served, app.store.object(&key).unwrap().0);

    // A tampered signature is refused.
    let tampered = if let Some(stripped) = token.strip_prefix('A') {
        format!("B{stripped}")
    } else {
        format!("A{}", &token[1..])
    };
    let uri = format!("/files/{key}?token={tampered}&exp={exp}");
    let resp = call(&app, make_request("GET", &uri, None, vec![], None)).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // A different expiry than the one signed is refused.
    let uri = format!("/files/{key}?token={token}&exp={}", exp + 1);
    let resp = call(&app, make_request("GET", &uri, None, vec![], None)).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Someone else's token cannot open my file: the token is bound to the
    // user id parsed from the path.
    let resp = call(
        &app,
        make_request("GET", "/files/sign", Some(&other_tok), vec![], None),
    )
    .await;
    let other_sig = body_json(resp).await;
    let other_exp = other_sig["expires_at"].as_i64().unwrap();
    let other_sig = other_sig["token"].as_str().unwrap().to_string();
    let uri = format!("/files/{key}?token={other_sig}&exp={other_exp}");
    let resp = call(&app, make_request("GET", &uri, None, vec![], None)).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // A missing token on a /files path is refused too.
    let resp = call(
        &app,
        make_request("GET", &format!("/files/{key}?width=96"), None, vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

async fn upload_rejects_non_image_bodies() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);

    // Text bytes under a .jpg name fail the magic-byte check.
    let (body, ct) = multipart("fake.jpg", "image/jpeg", b"this is not an image");
    let resp = call(
        &app,
        make_request("POST", "/upload", Some(&token), body, Some(&ct)),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    // A disallowed extension is refused before any decode.
    let (body, ct) = multipart("notes.txt", "text/plain", b"hello");
    let resp = call(
        &app,
        make_request("POST", "/upload", Some(&token), body, Some(&ct)),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // An empty multipart body is a plain 400.
    let (body, ct) = multipart("", "", &[]);
    let resp = call(
        &app,
        make_request("POST", "/upload", Some(&token), body, Some(&ct)),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

async fn tryon_rate_limit_kicks_in_at_five_per_minute() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);
    let item = create_item(&app, &token, "Worn Tee").await;

    let payload = serde_json::to_vec(&serde_json::json!({ "item_ids": [item] })).unwrap();
    let mut statuses = Vec::new();
    for _ in 0..6 {
        let resp = call(
            &app,
            make_request(
                "POST",
                "/outfits/generate-preview",
                Some(&token),
                payload.clone(),
                Some("application/json"),
            ),
        )
        .await;
        statuses.push(resp.status());
        drop(body_bytes(resp).await);
    }
    // The first five pass the limiter and die on the (absent) reference
    // photo; the sixth is a straight 429.
    let expected = [StatusCode::BAD_REQUEST; 5]
        .into_iter()
        .chain(std::iter::once(StatusCode::TOO_MANY_REQUESTS))
        .collect::<Vec<_>>();
    assert_eq!(statuses, expected);
}

// ---------------------------------------------------------------------------
// Try-on generation: provider dispatch, validation, storage + thumbnails
// ---------------------------------------------------------------------------

/// Update the caller's settings through the API (the same endpoint the
/// Settings view uses) and return the server's updated settings.
async fn set_settings(app: &TestApp, token: &str, fields: Value) -> Value {
    let payload = serde_json::to_vec(&fields).unwrap();
    let resp = call(
        app,
        make_request(
            "PUT",
            "/settings",
            Some(token),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "PUT /settings: {}",
        String::from_utf8(body_bytes(resp).await).unwrap_or_default()
    );
    body_json(resp).await
}

/// Create an item and attach a freshly uploaded test photo to it, so the
/// try-on handler finds a real `image_path` in storage.
async fn item_with_photo(app: &TestApp, token: &str, name: &str) -> String {
    let id = create_item(app, token, name).await;
    let key = upload_image(app, token).await;
    let payload = serde_json::to_vec(&serde_json::json!({ "image_path": key })).unwrap();
    let resp = call(
        app,
        make_request(
            "PATCH",
            &format!("/db/clothing-items/{id}"),
            Some(token),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "PATCH item {id}: {}",
        String::from_utf8(body_bytes(resp).await).unwrap_or_default()
    );
    id
}

/// A complete Gemini configuration: stored API key + reference photo.
fn gemini_settings(ref_key: &str) -> Value {
    serde_json::json!({
        "api_key": "gemini-test-key-abcdef123456",
        "reference_image_path": ref_key,
        "tryon_provider": "gemini"
    })
}

async fn tryon_gemini_generation_dispatches_and_stores() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    app.gemini.reset();
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);

    let ref_key = upload_image(&app, &token).await;
    let settings = set_settings(&app, &token, gemini_settings(&ref_key)).await;
    assert!(settings["has_api_key"].as_bool().unwrap());

    let item = item_with_photo(&app, &token, "Grey Hoodie").await;
    let payload = serde_json::to_vec(&serde_json::json!({ "item_ids": [item] })).unwrap();
    let resp = call(
        &app,
        make_request(
            "POST",
            "/outfits/generate-preview",
            Some(&token),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "gemini generate-preview: {}",
        String::from_utf8(body_bytes(resp).await).unwrap_or_default()
    );
    let image_path = body_json(resp).await["image_path"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        image_path.starts_with(&format!("users/{me}/")),
        "image_path: {image_path}"
    );
    assert!(image_path.ends_with("-tryon.jpg"));

    // The generated image landed in storage and decodes as a real JPEG.
    let (bytes, _) = app.store.object(&image_path).unwrap();
    assert!(
        image::load_from_memory(&bytes).is_ok(),
        "stored try-on image must decode"
    );
    // ...and got the same thumbnails as any other upload.
    for width in [96, 480] {
        assert!(
            app.store
                .object(&backend::upload::thumb_storage_key(&image_path, width))
                .is_some(),
            "missing {width}px thumbnail for {image_path}"
        );
    }

    // Dispatch went to Gemini exactly once, carrying the decrypted API key
    // and the reference photo bytes.
    let (count, key, req_body) = app.gemini.snapshot();
    assert_eq!(count, 1, "expected exactly one Gemini request");
    assert_eq!(key.as_deref(), Some("gemini-test-key-abcdef123456"));
    let ref_b64 = B64.encode(app.store.object(&ref_key).unwrap().0);
    assert!(
        req_body.as_deref().unwrap().contains(&ref_b64),
        "Gemini request must embed the reference photo"
    );
}

async fn tryon_qwen_generation_dispatches_and_stores() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    app.gemini.reset();
    app.qwen.reset();
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);

    let ref_key = upload_image(&app, &token).await;
    let settings = set_settings(
        &app,
        &token,
        serde_json::json!({
            "reference_image_path": ref_key,
            "tryon_provider": "qwen_local",
            "local_qwen_url": app.qwen_url
        }),
    )
    .await;
    assert_eq!(settings["tryon_provider"].as_str().unwrap(), "qwen_local");

    let item = item_with_photo(&app, &token, "Navy Trousers").await;
    let payload = serde_json::to_vec(&serde_json::json!({ "item_ids": [item] })).unwrap();
    let resp = call(
        &app,
        make_request(
            "POST",
            "/outfits/generate-preview",
            Some(&token),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "qwen generate-preview: {}",
        String::from_utf8(body_bytes(resp).await).unwrap_or_default()
    );
    let image_path = body_json(resp).await["image_path"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(image_path.starts_with(&format!("users/{me}/")));
    let (bytes, _) = app.store.object(&image_path).unwrap();
    assert!(
        image::load_from_memory(&bytes).is_ok(),
        "stored try-on image must decode"
    );

    // The request went to the local worker — not to Gemini — and carried the
    // prompt plus the reference photo first, then each garment.
    let (gemini_count, _, _) = app.gemini.snapshot();
    assert_eq!(gemini_count, 0, "qwen_local must not call Gemini");
    let reqs = app.qwen.requests();
    assert_eq!(reqs.len(), 1);
    let req = &reqs[0];
    assert!(
        req["prompt"].as_str().unwrap().contains("Navy Trousers"),
        "qwen prompt must name the garment"
    );
    let images = req["images_base64"].as_array().unwrap();
    assert_eq!(images.len(), 2, "reference photo + one garment");
    let ref_b64 = B64.encode(app.store.object(&ref_key).unwrap().0);
    assert_eq!(images[0].as_str().unwrap(), ref_b64);
}

/// Every 400 path the handler has, checked against a freshly configured user
/// each time so the per-minute generation cap never interferes.
async fn tryon_rejects_misconfigured_or_invalid_requests() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    app.gemini.reset();
    app.qwen.reset();

    async fn expect_400(app: &TestApp, token: &str, body: &Value) -> String {
        let resp = call(
            app,
            make_request(
                "POST",
                "/outfits/generate-preview",
                Some(token),
                serde_json::to_vec(body).unwrap(),
                Some("application/json"),
            ),
        )
        .await;
        let status = resp.status();
        let text = String::from_utf8(body_bytes(resp).await).unwrap();
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "expected 400, body: {text}"
        );
        text
    }

    // 1. No reference photo configured.
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);
    set_settings(
        &app,
        &token,
        serde_json::json!({ "api_key": "gemini-test-key-abcdef123456" }),
    )
    .await;
    let item = item_with_photo(&app, &token, "No Ref Tee").await;
    let body = expect_400(&app, &token, &serde_json::json!({ "item_ids": [item] })).await;
    assert!(body.contains("reference"), "no-reference error: {body}");

    // 2. Gemini selected but no API key stored.
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);
    let ref_key = upload_image(&app, &token).await;
    set_settings(
        &app,
        &token,
        serde_json::json!({ "reference_image_path": ref_key }),
    )
    .await;
    let item = item_with_photo(&app, &token, "No Key Tee").await;
    let body = expect_400(&app, &token, &serde_json::json!({ "item_ids": [item] })).await;
    assert!(body.contains("API key"), "no-key error: {body}");

    // 3. Item that is not in the caller's wardrobe.
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);
    let ref_key = upload_image(&app, &token).await;
    set_settings(&app, &token, gemini_settings(&ref_key)).await;
    let other = fresh_user(&app.pool).await;
    let other_token = jwt(&other, "authenticated", TEST_ISSUER, 3600);
    let foreign = item_with_photo(&app, &other_token, "Foreign Jeans").await;
    let body = expect_400(&app, &token, &serde_json::json!({ "item_ids": [foreign] })).await;
    assert!(body.contains("not found"), "foreign-item error: {body}");

    // 4. Item without a photo.
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);
    let ref_key = upload_image(&app, &token).await;
    set_settings(&app, &token, gemini_settings(&ref_key)).await;
    let bare = create_item(&app, &token, "No Photo Cap").await;
    let body = expect_400(&app, &token, &serde_json::json!({ "item_ids": [bare] })).await;
    assert!(body.contains("photo"), "no-photo error: {body}");

    // 5. Count bounds: zero and more than eight items.
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);
    let ref_key = upload_image(&app, &token).await;
    set_settings(&app, &token, gemini_settings(&ref_key)).await;
    let body = expect_400(&app, &token, &serde_json::json!({ "item_ids": [] })).await;
    assert!(body.contains("between 1 and 8"), "empty error: {body}");
    let ids: Vec<Value> = (0..9).map(|_| serde_json::json!(Uuid::new_v4())).collect();
    let body = expect_400(&app, &token, &serde_json::json!({ "item_ids": ids })).await;
    assert!(body.contains("between 1 and 8"), "over-cap error: {body}");

    // 6. Local Qwen selected with a URL the validator must refuse
    //    (non-loopback host). The settings endpoint refuses to *store* such a
    //    URL, so this path is only reachable with a row the API never wrote —
    //    seed it directly and let the handler's defense-in-depth check catch it.
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);
    let ref_key = upload_image(&app, &token).await;
    set_settings(
        &app,
        &token,
        serde_json::json!({
            "reference_image_path": ref_key,
            "tryon_provider": "qwen_local",
            "local_qwen_url": app.qwen_url
        }),
    )
    .await;
    sqlx::query(
        "UPDATE public.\"user-settings\" SET local_qwen_url = 'http://example.com:1234' \
         WHERE user_id = $1",
    )
    .bind(me)
    .execute(&app.pool)
    .await
    .unwrap();
    let body = expect_400(
        &app,
        &token,
        &serde_json::json!({ "item_ids": [Uuid::new_v4()] }),
    )
    .await;
    assert!(
        body.contains("Invalid local Qwen URL"),
        "bad-url error: {body}"
    );

    // None of the failed attempts may have reached a provider.
    let (gemini_count, _, _) = app.gemini.snapshot();
    assert_eq!(gemini_count, 0, "failed requests must not call Gemini");
    assert_eq!(
        app.qwen.requests().len(),
        0,
        "failed requests must not call Qwen"
    );
}

async fn list_endpoints_report_exact_totals_and_page() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);

    for i in 0..5 {
        create_item(&app, &token, &format!("Item {i}")).await;
    }

    // A short page reports the true total, not the page length.
    let resp = call(
        &app,
        make_request(
            "GET",
            "/db/clothing-items?limit=2&offset=0",
            Some(&token),
            vec![],
            None,
        ),
    )
    .await;
    assert_eq!(
        resp.headers()
            .get("X-Total-Count")
            .and_then(|v| v.to_str().ok()),
        Some("5")
    );
    let page: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert_eq!(page.len(), 2);

    // Walking the offset covers every row exactly once.
    let mut seen = 0;
    for offset in [2, 4] {
        let resp = call(
            &app,
            make_request(
                "GET",
                &format!("/db/clothing-items?limit=2&offset={offset}"),
                Some(&token),
                vec![],
                None,
            ),
        )
        .await;
        let page: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
        seen += page.len();
    }
    assert_eq!(seen, 3);
}

// ---------------------------------------------------------------------------
// M5: outfit editing, export/import, reference-data scoping and management
// ---------------------------------------------------------------------------

async fn patch_outfit(app: &TestApp, token: &str, id: &str, body: &Value) -> Value {
    let payload = serde_json::to_vec(body).unwrap();
    let resp = call(
        app,
        make_request(
            "PATCH",
            &format!("/outfits/{id}"),
            Some(token),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "PATCH failed: {}",
        String::from_utf8_lossy(&body_bytes(resp).await)
    );
    body_json(resp).await
}

async fn outfit_edit_updates_fields_and_wear_counts() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let me = fresh_user(&app.pool).await;
    let token = jwt(&me, "authenticated", TEST_ISSUER, 3600);

    let a = create_item(&app, &token, "Alpha Tshirt").await;
    let b = create_item(&app, &token, "Beta Jeans").await;
    let c = create_item(&app, &token, "Gamma Coat").await;

    let payload = serde_json::to_vec(
        &serde_json::json!({ "date": "2026-11-20", "notes": "original", "item_ids": [a, b] }),
    )
    .unwrap();
    let resp = call(
        &app,
        make_request(
            "POST",
            "/outfits",
            Some(&token),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let outfit_id = body_json(resp).await["id"].as_str().unwrap().to_string();

    // Swap B out for C: B loses its wear count, C gains one, A is untouched,
    // and the date and notes move with the request.
    let updated = patch_outfit(
        &app,
        &token,
        &outfit_id,
        &serde_json::json!({ "date": "2026-11-21", "notes": "edited", "item_ids": [a, c] }),
    )
    .await;
    assert_eq!(updated["date"].as_str().unwrap(), "2026-11-21");
    assert_eq!(updated["notes"].as_str().unwrap(), "edited");
    assert_eq!(
        updated["item_ids"].as_array().unwrap(),
        &vec![serde_json::json!(a), serde_json::json!(c)]
    );

    let resp = call(
        &app,
        make_request("GET", "/db/clothing-items", Some(&token), vec![], None),
    )
    .await;
    let rows: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    for row in &rows {
        let expected = match row["id"].as_str().unwrap() {
            id if id == a => 1,
            id if id == b => 0,
            _ => 1,
        };
        assert_eq!(
            row["wear_count"].as_i64(),
            Some(expected),
            "wear count for {}",
            row["id"]
        );
    }

    // Editing someone else's outfit, or a nonexistent one, is a 404.
    let other = fresh_user(&app.pool).await;
    let other_token = jwt(&other, "authenticated", TEST_ISSUER, 3600);
    let resp = call(
        &app,
        make_request(
            "PATCH",
            &format!("/outfits/{outfit_id}"),
            Some(&other_token),
            serde_json::to_vec(&serde_json::json!({ "date": "2026-11-21", "item_ids": [a] }))
                .unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = call(
        &app,
        make_request(
            "PATCH",
            "/outfits/00000000-0000-0000-0000-000000000000",
            Some(&token),
            serde_json::to_vec(&serde_json::json!({ "date": "2026-11-21", "item_ids": [] }))
                .unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Editing in an item the caller does not own is a 400.
    let foreign_item = create_item(&app, &other_token, "Foreign Item").await;
    let resp = call(
        &app,
        make_request(
            "PATCH",
            &format!("/outfits/{outfit_id}"),
            Some(&token),
            serde_json::to_vec(
                &serde_json::json!({ "date": "2026-11-21", "item_ids": [a, foreign_item] }),
            )
            .unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

async fn export_import_roundtrip_preserves_and_scopes_data() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let exporter = fresh_user(&app.pool).await;
    let exporter_token = jwt(&exporter, "authenticated", TEST_ISSUER, 3600);
    let importer = fresh_user(&app.pool).await;
    let importer_token = jwt(&importer, "authenticated", TEST_ISSUER, 3600);

    // The exporter assembles a small wardrobe: a color, a category, an item
    // with a photo in their own storage folder, and an outfit wearing it.
    let resp = call(
        &app,
        make_request(
            "POST",
            "/colors",
            Some(&exporter_token),
            serde_json::to_vec(&serde_json::json!({ "id": "Navy", "hex_value": "#00112b" }))
                .unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let row = body_json(resp).await;
    assert_eq!(row["id"].as_str().unwrap(), "navy");
    assert_eq!(row["user_id"].as_str().unwrap(), exporter.to_string());

    let resp = call(
        &app,
        make_request(
            "POST",
            "/clothing-categories",
            Some(&exporter_token),
            serde_json::to_vec(&serde_json::json!({ "name": "outerwear" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let category_id = body_json(resp).await["id"].as_i64().unwrap();

    let item = create_item(&app, &exporter_token, "Navy Jacket").await;
    let resp = call(
        &app,
        make_request(
            "PATCH",
            &format!("/db/clothing-items/{item}"),
            Some(&exporter_token),
            serde_json::to_vec(&serde_json::json!({
                "color_id": "navy",
                "category_id": category_id,
                "image_path": format!("users/{exporter}/abcd-shot.jpg"),
            }))
            .unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = call(
        &app,
        make_request(
            "POST",
            "/outfits",
            Some(&exporter_token),
            serde_json::to_vec(&serde_json::json!({ "date": "2026-11-10", "item_ids": [item] }))
                .unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // The export carries the whole wardrobe, scoped to the exporter.
    let resp = call(
        &app,
        make_request("GET", "/export", Some(&exporter_token), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let doc = body_json(resp).await;
    assert_eq!(doc["version"].as_i64().unwrap(), 1);
    assert_eq!(doc["user_id"].as_str().unwrap(), exporter.to_string());
    assert_eq!(doc["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        doc["outfits"].as_array().unwrap()[0]["item_ids"]
            .as_array()
            .unwrap(),
        &vec![serde_json::json!(item)]
    );
    assert!(doc["colors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == "navy"));
    assert!(doc["categories"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["name"] == "outerwear"));

    // The importer restores the document: rows are re-scoped to them, the
    // photo is dropped (it lives in the exporter's storage folder), and wear
    // counts come out of the links.
    let resp = call(
        &app,
        make_request(
            "POST",
            "/import",
            Some(&importer_token),
            serde_json::to_vec(&doc).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "import failed: {}",
        String::from_utf8_lossy(&body_bytes(resp).await)
    );
    let result = body_json(resp).await;
    assert_eq!(result["items_imported"].as_i64().unwrap(), 1);
    assert_eq!(result["outfits_imported"].as_i64().unwrap(), 1);
    assert_eq!(result["links_added"].as_i64().unwrap(), 1);
    assert_eq!(result["colors_created"].as_i64().unwrap(), 1);
    assert_eq!(result["categories_created"].as_i64().unwrap(), 1);

    let resp = call(
        &app,
        make_request(
            "GET",
            "/db/clothing-items",
            Some(&importer_token),
            vec![],
            None,
        ),
    )
    .await;
    let rows: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert_eq!(rows.len(), 1);
    // The item's document id belongs to the exporter's account, so the
    // importer's copy gets its own uuid (the old id is never reused).
    let imported_item = rows[0]["id"].as_str().unwrap().to_string();
    assert_ne!(imported_item, item);
    assert_eq!(rows[0]["user_id"].as_str().unwrap(), importer.to_string());
    assert_eq!(rows[0]["color_id"].as_str().unwrap(), "navy");
    assert!(
        rows[0]["image_path"].is_null(),
        "cross-account images must not carry over"
    );
    assert_eq!(rows[0]["wear_count"].as_i64(), Some(1));
    // The category resolved to the importer's own row, not the exporter's id.
    let resp = call(
        &app,
        make_request(
            "GET",
            "/db/clothing-categories",
            Some(&importer_token),
            vec![],
            None,
        ),
    )
    .await;
    let cats: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    let importer_cat = cats.iter().find(|c| c["name"] == "outerwear").unwrap();
    assert_eq!(rows[0]["category_id"].as_i64(), importer_cat["id"].as_i64());
    assert_ne!(rows[0]["category_id"].as_i64(), Some(category_id));

    let resp = call(
        &app,
        make_request("GET", "/outfits", Some(&importer_token), vec![], None),
    )
    .await;
    let outfits: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert_eq!(outfits.len(), 1);
    // The outfit's link was remapped onto the importer's own copy of the item.
    assert_eq!(
        outfits[0]["item_ids"].as_array().unwrap(),
        &vec![serde_json::json!(imported_item)]
    );

    // Re-importing the same document changes nothing: the item is recognised
    // by name, the outfit by its date/notes/item set, and the link pair
    // already exists.
    let resp = call(
        &app,
        make_request(
            "POST",
            "/import",
            Some(&importer_token),
            serde_json::to_vec(&doc).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    let result = body_json(resp).await;
    assert_eq!(result["items_imported"].as_i64().unwrap(), 0);
    assert_eq!(result["outfits_imported"].as_i64().unwrap(), 0);
    assert_eq!(result["links_added"].as_i64().unwrap(), 0);
    assert_eq!(result["colors_created"].as_i64().unwrap(), 0);
    assert_eq!(result["categories_created"].as_i64().unwrap(), 0);

    // Malformed documents are refused before anything is written.
    let mut broken = doc.clone();
    broken["outfits"][0]["item_ids"] = serde_json::json!(["11111111-1111-1111-1111-111111111111"]);
    let resp = call(
        &app,
        make_request(
            "POST",
            "/import",
            Some(&importer_token),
            serde_json::to_vec(&broken).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let mut dup = doc.clone();
    dup["items"] = serde_json::json!([dup["items"][0].clone(), dup["items"][0].clone()]);
    let resp = call(
        &app,
        make_request(
            "POST",
            "/import",
            Some(&importer_token),
            serde_json::to_vec(&dup).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

async fn reference_data_is_scoped_and_manageable() {
    let Some(app) = app().await else {
        eprintln!("skipped: no test database");
        return;
    };
    let a = fresh_user(&app.pool).await;
    let token_a = jwt(&a, "authenticated", TEST_ISSUER, 3600);
    let b = fresh_user(&app.pool).await;
    let token_b = jwt(&b, "authenticated", TEST_ISSUER, 3600);

    // A legacy shared color is visible to both users through the generic API.
    sqlx::query("INSERT INTO colors (id, hex_value) VALUES ('shared-blue', NULL)")
        .execute(&app.pool)
        .await
        .unwrap();
    for token in [&token_a, &token_b] {
        let resp = call(
            &app,
            make_request("GET", "/db/colors", Some(token), vec![], None),
        )
        .await;
        let rows: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
        assert!(rows.iter().any(|c| c["id"] == "shared-blue"));
    }

    // Each user owns a 'navy' row of their own; the two coexist, and neither
    // user's row shows up in the other's list.
    for (token, owner) in [(&token_a, a), (&token_b, b)] {
        let resp = call(
            &app,
            make_request(
                "POST",
                "/colors",
                Some(token),
                serde_json::to_vec(&serde_json::json!({ "id": "Navy" })).unwrap(),
                Some("application/json"),
            ),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let row = body_json(resp).await;
        assert_eq!(row["id"].as_str().unwrap(), "navy");
        assert_eq!(row["user_id"].as_str().unwrap(), owner.to_string());
    }
    // Per-user counts: the two 'navy' rows coexist, each owned by its creator
    // (a third 'navy' may exist from another scenario running in parallel).
    for owner in [a, b] {
        let count: (i64,) =
            sqlx::query_as("SELECT count(*) FROM colors WHERE id = 'navy' AND user_id = $1")
                .bind(owner)
                .fetch_one(&app.pool)
                .await
                .unwrap();
        assert_eq!(count.0, 1);
    }

    // Renaming an owned color re-points the owner's items; a slug that is
    // already taken by the owner is a 409; other users cannot touch it (404).
    let item = create_item(&app, &token_a, "Navy Tee").await;
    let _ = call(
        &app,
        make_request(
            "PATCH",
            &format!("/db/clothing-items/{item}"),
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "color_id": "navy" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    let _ = call(
        &app,
        make_request(
            "POST",
            "/colors",
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "id": "midnight" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    let resp = call(
        &app,
        make_request(
            "PATCH",
            "/colors/navy",
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "id": "midnight" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let resp = call(
        &app,
        make_request(
            "PATCH",
            "/colors/navy",
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "id": "deep-night" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = call(
        &app,
        make_request("GET", "/db/clothing-items", Some(&token_a), vec![], None),
    )
    .await;
    let rows: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert_eq!(rows[0]["color_id"].as_str().unwrap(), "deep-night");

    let resp = call(
        &app,
        make_request(
            "PATCH",
            "/colors/deep-night",
            Some(&token_b),
            serde_json::to_vec(&serde_json::json!({ "id": "stolen" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Deleting an owned color nulls the owner's references and reports them.
    let resp = call(
        &app,
        make_request("DELETE", "/colors/deep-night", Some(&token_a), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["items_affected"].as_i64().unwrap(), 1);
    let resp = call(
        &app,
        make_request("GET", "/db/clothing-items", Some(&token_a), vec![], None),
    )
    .await;
    let rows: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert!(rows[0]["color_id"].is_null());

    // A shared color in use anywhere cannot be deleted; with no references it
    // can, by anyone who can see it.
    let _ = call(
        &app,
        make_request(
            "PATCH",
            &format!("/db/clothing-items/{item}"),
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "color_id": "shared-blue" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    let resp = call(
        &app,
        make_request(
            "DELETE",
            "/colors/shared-blue",
            Some(&token_a),
            vec![],
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let _ = call(
        &app,
        make_request(
            "PATCH",
            &format!("/db/clothing-items/{item}"),
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "color_id": null })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    let resp = call(
        &app,
        make_request(
            "DELETE",
            "/colors/shared-blue",
            Some(&token_b),
            vec![],
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Categories: rename keeps the id (item references untouched), delete
    // removes owned subcategories and nulls the owner's items, and a
    // category with remaining subcategories refuses to go.
    let resp = call(
        &app,
        make_request(
            "POST",
            "/clothing-categories",
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "name": "outerwear" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let category_id = body_json(resp).await["id"].as_i64().unwrap();

    let resp = call(
        &app,
        make_request(
            "GET",
            "/db/clothing-categories",
            Some(&token_b),
            vec![],
            None,
        ),
    )
    .await;
    let rows: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert!(!rows.iter().any(|c| c["name"] == "outerwear"));

    let item2 = create_item(&app, &token_a, "Rain Shell").await;
    let _ = call(
        &app,
        make_request(
            "PATCH",
            &format!("/db/clothing-items/{item2}"),
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "category_id": category_id })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    let resp = call(
        &app,
        make_request(
            "PATCH",
            &format!("/clothing-categories/{category_id}"),
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "name": "jackets" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["id"].as_i64().unwrap(), category_id);
    let resp = call(
        &app,
        make_request("GET", "/db/clothing-items", Some(&token_a), vec![], None),
    )
    .await;
    let rows: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert_eq!(rows[0]["category_id"].as_i64().unwrap(), category_id);

    // A subcategory under it: deleting the category takes the subcategory and
    // clears both references from the owner's item.
    let resp = call(
        &app,
        make_request(
            "POST",
            "/clothing-subcategories",
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "name": "parka", "category_id": category_id }))
                .unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let sub_id = body_json(resp).await["id"].as_i64().unwrap();
    let _ = call(
        &app,
        make_request(
            "PATCH",
            &format!("/db/clothing-items/{item2}"),
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "subcategory_id": sub_id })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    let resp = call(
        &app,
        make_request(
            "DELETE",
            &format!("/clothing-categories/{category_id}"),
            Some(&token_a),
            vec![],
            None,
        ),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "delete category failed: {}",
        String::from_utf8_lossy(&body_bytes(resp).await)
    );
    let result = body_json(resp).await;
    assert_eq!(result["items_affected"].as_i64().unwrap(), 1);
    assert_eq!(result["subcategories_deleted"].as_i64().unwrap(), 1);
    let resp = call(
        &app,
        make_request("GET", "/db/clothing-items", Some(&token_a), vec![], None),
    )
    .await;
    let rows: Vec<Value> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert!(rows[0]["category_id"].is_null());
    assert!(rows[0]["subcategory_id"].is_null());

    // A subcategory can move to another of the owner's categories, but not to
    // a category owned by someone else.
    let resp = call(
        &app,
        make_request(
            "POST",
            "/clothing-categories",
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "name": "bottoms" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    let bottoms = body_json(resp).await["id"].as_i64().unwrap();
    let resp = call(
        &app,
        make_request(
            "POST",
            "/clothing-categories",
            Some(&token_b),
            serde_json::to_vec(&serde_json::json!({ "name": "b-outerwear" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    let b_category = body_json(resp).await["id"].as_i64().unwrap();
    let resp = call(
        &app,
        make_request(
            "POST",
            "/clothing-subcategories",
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "name": "jeans", "category_id": bottoms }))
                .unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    let jeans = body_json(resp).await["id"].as_i64().unwrap();
    let resp = call(
        &app,
        make_request(
            "PATCH",
            &format!("/clothing-subcategories/{jeans}"),
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "name": "denim", "category_id": b_category }))
                .unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = call(
        &app,
        make_request(
            "PATCH",
            &format!("/clothing-subcategories/{jeans}"),
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "name": "denim", "category_id": bottoms }))
                .unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["name"].as_str().unwrap(), "denim");

    // Brands: created through the dedicated endpoint (user_id forced),
    // managed through it and the rename/delete endpoints, scoped the same
    // way as the other reference tables.
    let resp = call(
        &app,
        make_request(
            "POST",
            "/brands",
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "name": "Acme" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let brand_row = body_json(resp).await;
    let brand = brand_row["id"].as_str().unwrap().to_string();
    assert_eq!(brand_row["user_id"].as_str().unwrap(), a.to_string());
    let resp = call(
        &app,
        make_request(
            "PATCH",
            &format!("/brands/{brand}"),
            Some(&token_b),
            serde_json::to_vec(&serde_json::json!({ "name": "Stolen" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let item3 = create_item(&app, &token_a, "Acme Boots").await;
    let _ = call(
        &app,
        make_request(
            "PATCH",
            &format!("/db/clothing-items/{item3}"),
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "brand_id": brand })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    let resp = call(
        &app,
        make_request(
            "DELETE",
            &format!("/brands/{brand}"),
            Some(&token_a),
            vec![],
            None,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["items_affected"].as_i64().unwrap(), 1);

    // The usage endpoint reports per-row reference counts for the caller.
    let _ = call(
        &app,
        make_request(
            "POST",
            "/colors",
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "id": "counted" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    let _ = call(
        &app,
        make_request(
            "PATCH",
            &format!("/db/clothing-items/{item3}"),
            Some(&token_a),
            serde_json::to_vec(&serde_json::json!({ "color_id": "counted" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    let resp = call(
        &app,
        make_request("GET", "/reference-usage", Some(&token_a), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let usage = body_json(resp).await;
    assert_eq!(usage["colors"]["counted"].as_i64().unwrap(), 1);
    assert_eq!(
        usage["categories"]["bottoms".to_string()].as_i64().unwrap(),
        0
    );
    // Unreferenced but still visible to the owner.
    assert_eq!(usage["colors"]["midnight"].as_i64().unwrap(), 0);
    // B's own color never appears in A's usage counts.
    let _ = call(
        &app,
        make_request(
            "POST",
            "/colors",
            Some(&token_b),
            serde_json::to_vec(&serde_json::json!({ "id": "b-only" })).unwrap(),
            Some("application/json"),
        ),
    )
    .await;
    assert!(!usage["colors"].as_object().unwrap().contains_key("b-only"));
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Every scenario above is a plain `async fn` driven by **one** runtime here.
///
/// This matters: the shared `TestApp` owns a sqlx pool and a spawned mock
/// storage server, both of which are bound to the runtime that built them.
/// If each scenario were a `#[tokio::test]`, the harness would give each its
/// own runtime — the first scenario's runtime would build the app, and the
/// moment that test finished, the shared pool and mock server would be torn
/// down out from under every later scenario. One runtime keeps them alive for
/// the whole suite.
#[test]
fn integration_suite() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        // Build the shared app on this runtime before any scenario runs.
        if app().await.is_none() {
            eprintln!("skipped: no test database");
            return;
        }

        let mut failed: Vec<&str> = vec![];

        macro_rules! scenario {
            ($name:expr, $future:expr) => {
                let name = $name;
                match tokio::spawn($future).await {
                    Ok(()) => eprintln!("ok   {name}"),
                    Err(_) => {
                        eprintln!("FAIL {name}");
                        failed.push(name);
                    }
                }
            };
        }

        scenario!("healthz", healthz_reports_ok_and_sets_security_headers());
        scenario!("headers", every_response_carries_the_hygiene_headers());
        scenario!("auth", rejects_unauthenticated_and_invalid_tokens());
        scenario!(
            "tables_listing",
            tables_listing_shares_the_schema_and_hides_blocked_tables()
        );
        scenario!(
            "crud_scoping",
            generic_crud_scopes_everything_to_the_caller()
        );
        scenario!(
            "insert_ownership",
            insert_forces_caller_ownership_and_generates_the_id()
        );
        scenario!("blocked_tables", blocked_tables_reject_generic_access());
        scenario!(
            "shared_tables_read_only",
            shared_reference_tables_are_read_only()
        );
        scenario!(
            "outfit_lifecycle",
            outfit_lifecycle_keeps_wear_counts_in_sync()
        );
        scenario!(
            "outfit_validation",
            outfit_validation_rejects_foreign_and_malformed_items()
        );
        scenario!("outfit_edit", outfit_edit_updates_fields_and_wear_counts());
        scenario!(
            "export_import",
            export_import_roundtrip_preserves_and_scopes_data()
        );
        scenario!("reference_data", reference_data_is_scoped_and_manageable());
        scenario!("settings", settings_roundtrip_encrypts_api_keys());
        scenario!(
            "discord_code",
            discord_link_code_is_stored_with_a_15_minute_expiry()
        );
        scenario!("discord_unlink", unlink_discord_clears_the_pairing());
        scenario!(
            "upload_serve",
            upload_and_serve_respect_auth_and_ownership()
        );
        scenario!("file_tokens", signed_file_tokens_gate_image_serving());
        scenario!("upload_validation", upload_rejects_non_image_bodies());
        scenario!(
            "tryon_rate_limit",
            tryon_rate_limit_kicks_in_at_five_per_minute()
        );
        scenario!(
            "tryon_gemini",
            tryon_gemini_generation_dispatches_and_stores()
        );
        scenario!("tryon_qwen", tryon_qwen_generation_dispatches_and_stores());
        scenario!(
            "tryon_validation",
            tryon_rejects_misconfigured_or_invalid_requests()
        );
        scenario!("paging", list_endpoints_report_exact_totals_and_page());

        assert!(
            failed.is_empty(),
            "{} scenario(s) failed: {failed:?}",
            failed.len()
        );
    });
}
