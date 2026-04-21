use graphium::serde::{Serialize, Deserialize};

#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow, Serialize, Deserialize)]
pub struct Product {
    pub id: i64,
    pub name: String,
    pub price: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewProduct {
    pub name: String,
    pub price: i64,
}

