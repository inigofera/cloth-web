//! End-to-end integration tests: a real Postgres (the full migration chain
//! applied to a clean schema) plus an in-process mock of the Supabase
//! Storage API. The app under test is built with the same `build_router`
//! the binary uses, so these tests exercise exactly what ships.
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
use axum::routing::get;
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
// Shared app: one database reset + one migration pass + one mock, shared by
// every test in this binary.
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct TestApp {
    router: Router,
    pool: Pool<Postgres>,
    store: MockStore,
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
                sqlx::raw_sql(&sql).execute(&pool).await.unwrap();
            }

            let store = MockStore::new();
            let base_url = start_storage_mock(store.clone()).await;

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
            });

            TestApp {
                router: build_router(state),
                pool,
                store,
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

    // Reading a shared table works.
    let resp = call(
        &app,
        make_request("GET", "/db/colors", Some(&token), vec![], None),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // But writing through the generic API is forbidden: colors have no user_id.
    let payload =
        serde_json::to_vec(&serde_json::json!({ "id": "hacked", "hex_value": "#ff0000" })).unwrap();
    let resp = call(
        &app,
        make_request(
            "POST",
            "/db/colors",
            Some(&token),
            payload,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
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
        scenario!("paging", list_endpoints_report_exact_totals_and_page());

        assert!(
            failed.is_empty(),
            "{} scenario(s) failed: {failed:?}",
            failed.len()
        );
    });
}
