#[derive(Clone)]
pub struct Context {
    pub pool: sqlx::PgPool,
}

impl Context {
    pub async fn new() -> Self {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(&database_url)
            .await
            .map_err(|e| format!("postgres connect failed: {e}"))
            .unwrap();

        Self {
            pool,
        }
    }
}
