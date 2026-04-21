use graphium::serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateProduct {
    pub name: Option<String>,
    pub price: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteResult {
    pub deleted: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListProductsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
