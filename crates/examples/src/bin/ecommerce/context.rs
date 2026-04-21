#[derive(Clone)]
pub struct Context {
    pub pool: sqlx::PgPool,
    pub product_input: crate::models::NewProduct,
    pub product: crate::models::Product,
}

impl Context {
    pub async fn new() -> Self {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect("postgres://postgres:postgres@localhost:5432/postgres")
            .await
            .map_err(|e| format!("postgres connect failed: {e}"))
            .unwrap();

        Self {
            pool,
            product_input: crate::models::NewProduct {
                name: String::new(),
                price: 0,
            },
            product: crate::models::Product {
                id: 0,
                name: String::new(),
                price: 0,
            },
        }
    }
}
