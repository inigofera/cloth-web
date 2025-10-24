use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

pub async fn connect_db() -> Pool<Postgres> {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .expect("failed to connect to Supabase Postgres")
}
