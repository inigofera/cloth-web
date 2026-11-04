use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

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
