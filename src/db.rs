use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::routes::{ColumnInfo, quote_ident};

pub async fn connect_db() -> Pool<Postgres> {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .expect("failed to connect to Supabase Postgres");

    // Ensure Discord integration columns exist on user-settings
    let _ = sqlx::query(
        r#"
        ALTER TABLE public."user-settings"
            ADD COLUMN IF NOT EXISTS discord_user_id text UNIQUE,
            ADD COLUMN IF NOT EXISTS discord_link_code text,
            ADD COLUMN IF NOT EXISTS discord_link_expires_at timestamptz;
        "#,
    )
    .execute(&pool)
    .await;

    pool
}

// The public tables and their shapes, read once at startup and carried in AppState, so
// the generic CRUD endpoints stop asking Postgres the same questions on every request.
//
// Those handlers used to ask three or four questions before the real work even started
// (`is_allowed_table`, `table_has_user_id`, `table_has_column`, `table_column_names`),
// and those answers never change unless a migration runs: a table that does not exist
// pays for a schema read before it gets a 404. The schema is the one thing in this
// process that cannot be a compile-time constant, because these tables belong to
// whoever ran the migrations, but it must not be per-request either - load it once and
// the handler asks a map instead of the wire.
//
// The snapshot also carries the SQL each table is read and written with. `sqlx`
// re-parses and re-binds whatever query text it is handed, so deciding the shape of a
// statement per table at startup, rather than rebuilding it per request, is what lets a
// write request send exactly one statement and lets the paging total ride along with the
// rows instead of scanning the same rows a second time.

#[derive(Clone)]
pub struct ColumnFact {
    pub column_name: String,
    pub data_type: String,
}

#[derive(Clone)]
pub struct TableFacts {
    pub name: String,
    pub quoted_table: String,
    // Columns in Postgres' ordinal order: an insert rebuilds a row from a client body,
    // so it has to name every column it may copy, and `/db` lists them for the UI.
    pub columns: Vec<ColumnFact>,
    pub has_user_id: bool,
    pub has_updated_at: bool,
    pub has_created_at: bool,
    // How a row's id is filtered, by the type Postgres reports for that column. The CRUD
    // handlers used `id::text = $1`, which casts every row's id and hides the primary-key
    // index from a PATCH or DELETE; the cast belongs on the value the handler binds
    // instead, which leaves the index usable.
    pub id_filter: String,
    pub id_filter_aliased: String,
    // `uuid` when the id column Postgres reports is a uuid: the handler then binds a
    // parsed value, so a malformed id cannot name a row and no per-row cast is needed.
    pub id_kind: String,
    // `ORDER BY` fragment for a paged read of this table.
    pub order_sql: String,
    // Read one page of this table's rows for one user: $1 user, $2 limit.
    // `COUNT(*) OVER ()` is evaluated after the row filter and before the page is cut,
    // so the total the UI reads from `X-Total-Count` costs no second scan of the rows
    // this page is about to serialise, and a capped page still reports the true total.
    pub scoped_rows_sql: String,
    // Read a page of a shared reference table: $1 limit. Those tables hold one small
    // list (colors, categories, ...) the UI reads whole.
    pub shared_rows_sql: String,
    pub insert_sql: String,
    pub delete_sql: String,
    // Update one row: $1 id, $2 JSON body, $3 user. The SET list is the only part that
    // varies between requests, so it is spliced between these two halves.
    pub update_head: String,
    pub update_tail: String,
}

impl TableFacts {
    pub fn has_column(&self, column: String) -> bool {
        self.columns.iter().any(|fact| fact.column_name == column)
    }

    pub fn column_names(&self) -> Vec<String> {
        self.columns.iter().map(|fact| fact.column_name.clone()).collect()
    }

    pub fn covers_columns(&self, keys: &Vec<String>) -> bool {
        keys.iter().all(|key| self.has_column(key.clone()))
    }
}

// The order a paged read cuts through. `created_at` is what the wardrobe lists page
// through, newest first, and the row's own id breaks ties so the page cut is a stable
// total order: without a tie-break, rows sharing a timestamp can straddle a page
// boundary and the completeness notice the UI derives from `X-Total-Count` reads a
// window that moves. The shipped order, `ORDER BY 1`, sorted on the `to_jsonb(t)` blob,
// which asks Postgres to serialise every row and then compare large strings; it stays
// only as the fallback for a table with neither column, where a fallback order cannot
// lie because those tables are read whole. A table with a `created_at` but no id column
// has no cheap tie-break either, so it keeps that shipped order rather than an unstable one.
pub fn order_sql_for(has_created_at: bool, has_id: bool) -> String {
    if has_created_at && has_id {
        "ORDER BY t.\"created_at\" DESC NULLS LAST, t.id ASC".to_string()
    } else if has_created_at {
        "ORDER BY 1".to_string()
    } else if has_id {
        "ORDER BY t.id ASC".to_string()
    } else {
        "ORDER BY 1".to_string()
    }
}

// The cast belongs on the value the handler binds, not on the column. A table whose id
// Postgres reports as something else (a text slug, an integer category id) keeps the
// text comparison the handlers shipped with, which is correct for those tables even
// though it cannot use an index; those tables hold a handful of rows.
pub fn id_filters_for(data_type: String) -> (String, String) {
    if data_type == "uuid" || data_type == "text" {
        ("id = $1".to_string(), "t.id = $1".to_string())
    } else {
        ("id::text = $1".to_string(), "t.id::text = $1".to_string())
    }
}

// The column list an insert copies out of the posted JSON. `created_at` and `updated_at`
// fall back to `now()` so a body that omits a timestamp still gets one.
pub fn insert_select_list_for(columns: &Vec<ColumnFact>) -> String {
    columns
        .iter()
        .map(|fact|
            match fact.column_name.as_str() {
                "created_at" | "updated_at" =>
                    format!("COALESCE(r.\"{}\", now()) AS \"{}\"", fact.column_name, fact.column_name),
                _ => format!("r.\"{}\"", fact.column_name),
            }
        )
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn facts_for(table: String, columns: Vec<ColumnFact>) -> TableFacts {
    let quoted_table = quote_ident(&table);
    let has_user_id = columns.iter().any(|fact| fact.column_name == "user_id");
    let has_updated_at = columns.iter().any(|fact| fact.column_name == "updated_at");
    let has_created_at = columns.iter().any(|fact| fact.column_name == "created_at");
    let has_id = columns.iter().any(|fact| fact.column_name == "id");
    let id_data_type = columns
        .iter()
        .find(|fact| fact.column_name == "id")
        .map(|fact| fact.data_type.clone())
        .unwrap_or("".to_string());
    let id_kind = if id_data_type == "uuid" { "uuid".to_string() } else { "other".to_string() };
    let (id_filter, id_filter_aliased) = id_filters_for(id_data_type);
    let order_sql = order_sql_for(has_created_at, has_id);
    let select_list = if columns.is_empty() {
        "r.*".to_string()
    } else {
        insert_select_list_for(&columns)
    };

    // Every statement shape this table can be read or written with, rendered once here
    // rather than rebuilt on every request.
    // Paging is real here, not just a cap: `$3`/`$2` is the row to start at, so a
    // client can walk a table in pages while `COUNT(*) OVER ()` still reports the
    // rows that exist rather than the rows this page happens to include.
    let scoped_rows_sql = format!(
        "SELECT to_jsonb(t) AS row, COUNT(*) OVER () AS total FROM public.\"{quoted_table}\" t WHERE t.user_id = $1 {order_sql} LIMIT $2 OFFSET $3",
    );
    let shared_rows_sql = format!(
        "SELECT to_jsonb(t) AS row, COUNT(*) OVER () AS total FROM public.\"{quoted_table}\" t {order_sql} LIMIT $1 OFFSET $2",
    );
    let insert_sql = format!(
        "WITH r AS (\n  SELECT * FROM json_populate_record(NULL::public.\"{quoted_table}\", $1::json)\n)\n,\ninserted AS (\n  INSERT INTO public.\"{quoted_table}\"\n  SELECT {select_list} FROM r\n  RETURNING *\n)\nSELECT to_jsonb(inserted.*) AS row FROM inserted",
    );
    let delete_sql = format!(
        "WITH deleted AS (\n  DELETE FROM public.\"{quoted_table}\" WHERE {id_filter} AND user_id = $2 RETURNING *\n)\nSELECT to_jsonb(deleted.*) AS row FROM deleted",
    );
    let update_head = format!(
        "WITH r AS (\n  SELECT * FROM json_populate_record(NULL::public.\"{quoted_table}\", $2::json)\n)\nUPDATE public.\"{quoted_table}\" AS t\nSET ",
    );
    let update_tail = format!(
        "\nFROM r\nWHERE {id_filter_aliased} AND t.user_id = $3\nRETURNING to_jsonb(t) AS row",
    );

    TableFacts {
        name: table,
        quoted_table: quoted_table,
        columns: columns,
        has_user_id: has_user_id,
        has_updated_at: has_updated_at,
        has_created_at: has_created_at,
        id_filter: id_filter,
        id_filter_aliased: id_filter_aliased,
        id_kind: id_kind,
        order_sql: order_sql,
        scoped_rows_sql: scoped_rows_sql,
        shared_rows_sql: shared_rows_sql,
        insert_sql: insert_sql,
        delete_sql: delete_sql,
        update_head: update_head,
        update_tail: update_tail,
    }
}

// Everything the CRUD endpoints need to know about the public schema, in one statement.
// The old code asked for these facts one question per request; one read covers all of
// them at once, and it happens at startup rather than per request.
pub const SCHEMA_SQL: &str =
    "SELECT table_name, column_name, data_type FROM information_schema.columns WHERE table_schema = 'public' ORDER BY table_name, ordinal_position";

// Tables this snapshot knows about, plus the names a schema read never found: without
// remembering the second set, a mistyped or blocked table name would make every request
// re-read the schema.
pub struct SchemaSnapshot {
    pub by_name: HashMap<String, TableFacts>,
    // Tables in schema order, for the `GET /db` listing.
    pub listing: Vec<TableFacts>,
    pub unknown: HashMap<String, bool>,
}

impl SchemaSnapshot {
    pub fn empty() -> SchemaSnapshot {
        SchemaSnapshot {
            by_name: HashMap::new(),
            listing: vec![],
            unknown: HashMap::new(),
        }
    }
}

pub struct SchemaCache {
    pub state: Mutex<SchemaSnapshot>,
}

impl SchemaCache {
    pub fn new() -> SchemaCache {
        SchemaCache { state: Mutex::new(SchemaSnapshot::empty()) }
    }

    // Read the whole public schema and replace the snapshot with it: migration-time truth
    // costs one schema read, and the requests that follow go back to being answered from
    // memory. The stale `unknown` names are dropped here, so a table a migration added
    // stops being remembered as missing.
    pub async fn refresh(&self, pool: &sqlx::Pool<sqlx::Postgres>) -> usize {
        let rows: Vec<ColumnInfo> = sqlx::query_as::<_, ColumnInfo>(SCHEMA_SQL)
            .fetch_all(pool)
            .await
            .unwrap_or_else(|error| {
                eprintln!("db schema read error: {error}");
                vec![]
            });

        // One row per column, ordered by table then column. Tables come out in schema
        // order, so the listing the UI reads matches what a fresh schema read would say.
        let mut listing: Vec<TableFacts> = vec![];
        let mut by_name: HashMap<String, TableFacts> = HashMap::new();
        let mut pending: Vec<ColumnFact> = vec![];
        let mut current: String = "".to_string();
        for row in &rows {
            let column = ColumnFact {
                column_name: row.column_name.clone(),
                data_type: row.data_type.clone(),
            };
            if current != row.table_name {
                if current != "" {
                    let facts = facts_for(current, pending);
                    listing.push(facts.clone());
                    by_name.insert(facts.name.clone(), facts);
                };
                current = row.table_name.clone();
                pending = vec![column];
            } else {
                pending.push(column);
            };
        };
        if current != "" {
            let facts = facts_for(current, pending);
            listing.push(facts.clone());
            by_name.insert(facts.name.clone(), facts);
        };

        let table_count = listing.len();
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.by_name = by_name;
        state.listing = listing;
        state.unknown = HashMap::new();
        table_count
    }

    // Facts for one table out of the snapshot, without touching the wire.
    pub fn probe(&self, table: String) -> Option<TableFacts> {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match state.by_name.get(&table) {
            None => None,
            Some(facts) => Some(facts.clone()),
        }
    }

    // Facts for one table, with exactly one schema read behind a name the snapshot never
    // heard of - that is what a migration looks like, and the next request for that name
    // is answered from memory either way. A name the refreshed schema still does not
    // know is remembered as unknown, so it cannot make every request re-read the schema.
    // A name the last schema read did not find. Kept as its own short lock so the refresh
    // that follows is never waiting on a lock this method still holds.
    pub fn unknown_for(&self, table: String) -> bool {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.unknown.contains_key(&table)
    }

    pub async fn facts(&self, pool: &sqlx::Pool<sqlx::Postgres>, table: String) -> Option<TableFacts> {
        match self.probe(table.clone()) {
            Some(facts) => return Some(facts),
            None => {},
        };

        if self.unknown_for(table.clone()) {
            return None;
        };

        let tables = self.refresh(pool).await;
        if tables == 0 {
            return None;
        };

        match self.probe(table.clone()) {
            Some(facts) => Some(facts),
            None => {
                let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                state.unknown.insert(table, true);
                None
            },
        }
    }

    // Facts for a write whose body may name a column this snapshot never heard of - a
    // migration that added a column. Trusting the snapshot here would reject a column
    // that exists, so one schema read runs first when the body names something unknown;
    // that costs one query for that request rather than three or four for every request.
    pub async fn facts_for_write(
        &self,
        pool: &sqlx::Pool<sqlx::Postgres>,
        table: String,
        keys: &Vec<String>,
    ) -> Option<TableFacts> {
        let facts = self.facts(pool, table.clone()).await;
        match facts {
            Some(found) => {
                if found.covers_columns(keys) {
                    return Some(found);
                };
            },
            None => return None,
        };

        self.refresh(pool).await;
        self.probe(table)
    }

    // Every table the snapshot knows, in schema order, for the `GET /db` listing.
    pub fn snapshot(&self) -> Vec<TableFacts> {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.listing.clone()
    }
}

#[cfg(test)]
pub mod schema_tests {
    use super::facts_for;
    use super::order_sql_for;
    use super::id_filters_for;
    use super::insert_select_list_for;
    use super::ColumnFact;

    fn id_of(kind: String) -> ColumnFact {
        ColumnFact { column_name: "id".to_string(), data_type: kind }
    }

    fn created_at() -> ColumnFact {
        ColumnFact { column_name: "created_at".to_string(), data_type: "timestamp with time zone".to_string() }
    }

    fn owner() -> ColumnFact {
        ColumnFact { column_name: "user_id".to_string(), data_type: "uuid".to_string() }
    }

    #[test]
    fn uuid_scoped_read_carries_the_total() {
        let facts = facts_for(
            "clothing-items".to_string(),
            vec![id_of("uuid".to_string()), created_at(), owner()],
        );

        assert!(facts.has_user_id);
        assert!(facts.id_kind == "uuid");
        assert!(facts.scoped_rows_sql.find("COUNT(*) OVER () AS total") != None);
        assert!(facts.scoped_rows_sql.find("WHERE t.user_id = $1") != None);
        assert!(facts.scoped_rows_sql.find("ORDER BY t.\"created_at\" DESC NULLS LAST, t.id ASC") != None);
        assert!(facts.scoped_rows_sql.find("LIMIT $2 OFFSET $3") != None);
        // The paging order has to be stable: a page boundary may only skip rows the previous
        // page already answered.
        assert!(facts.order_sql.find(", t.id ASC") != None);
    }

    #[test]
    fn uuid_key_filter_keeps_the_index_usable() {
        let filters = id_filters_for("uuid".to_string());
        assert!(filters.0 == "id = $1");
        assert!(filters.1 == "t.id = $1");

        let facts = facts_for(
            "outfits".to_string(),
            vec![id_of("uuid".to_string()), owner()],
        );
        assert!(facts.delete_sql.find("WHERE id = $1 AND user_id = $2") != None);
        assert!(facts.update_tail.find("WHERE t.id = $1 AND t.user_id = $3") != None);
    }

    #[test]
    fn slug_and_untyped_ids_still_answer() {
        // `colors` is shared reference data keyed by a text slug: no user scoping, and the
        // parameter carries the cast so the primary key index still answers the lookup.
        let colors = facts_for(
            "colors".to_string(),
            vec![id_of("text".to_string()), ColumnFact { column_name: "hex".to_string(), data_type: "text".to_string() }],
        );
        assert!(!colors.has_user_id);
        assert!(colors.shared_rows_sql.find("COUNT(*) OVER () AS total") != None);
        assert!(colors.id_filter == "id = $1");

        // An id column of some other shape falls back to comparing its rendered text.
        let unkeyed = facts_for(
            "categories".to_string(),
            vec![ColumnFact { column_name: "category_id".to_string(), data_type: "int8".to_string() }, owner()],
        );
        assert!(unkeyed.id_kind == "other");
        assert!(unkeyed.id_filter == "id::text = $1");
        // With no created_at and no id column there is nothing cheap to order by: keep the
        // documented column-position ordering rather than inventing an unstable one.
        assert!(unkeyed.order_sql == order_sql_for(false, false));
        assert!(unkeyed.order_sql.find("ORDER BY 1") != None);
    }

    #[test]
    fn insert_gives_created_rows_the_timestamps() {
        let select_list = insert_select_list_for(
            &vec![id_of("uuid".to_string()), created_at(), ColumnFact { column_name: "updated_at".to_string(), data_type: "timestamp".to_string() }, owner()],
        );
        assert!(select_list.find("COALESCE(r.\"created_at\", now()) AS \"created_at\"") != None);
        assert!(select_list.find("COALESCE(r.\"updated_at\", now()) AS \"updated_at\"") != None);
        assert!(select_list.find("COALESCE(r.\"user_id\", now())") == None);
    }

    #[test]
    fn update_statement_is_split_around_the_set_list() {
        let facts = facts_for(
            "user-settings".to_string(),
            vec![id_of("uuid".to_string()), ColumnFact { column_name: "updated_at".to_string(), data_type: "timestamp".to_string() }],
        );
        assert!(facts.has_updated_at);
        assert!(facts.update_head.find("UPDATE public.\"user-settings\" AS t") != None);
        assert!(facts.update_head.find("SET") != None);
        assert!(facts.update_tail.find("RETURNING to_jsonb(t) AS row") != None);
    }
}
