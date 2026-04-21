#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct Product {
    pub id: i64,
    pub name: String,
    pub price: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewProduct {
    pub name: String,
    pub price: i64,
}

