use crate::context::Context;
use crate::models::{ApiError, DeleteResult, Product, UpdateProduct};
use crate::nodes::product::*;
use axum::Json;
use graphium::graph;

graph! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    async CreateProductGraph<Context>(name: String, price: String) -> (product_dto: Result<Json<Product>, ApiError>) {
        GetProductInput(name, price) -> (product_input) >>
        ValidateProductInputData(product_input) -> (product_input) >>
        CheckProductDoesNotExist(product_input) -> (product_input) >>
        ProductCreate(product_input) -> (product) >>
        SerializeProduct(product) -> (product_dto)
    }
}

graph! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    async GetProductGraph<Context>(product_id: i64) -> (product_dto: Result<Json<Product>, ApiError>) {
        ProductGetById(product_id) -> product_result >>
        UnwrapResultOptionProduct(product_result) -> product >>
        SerializeProduct(product) -> (product_dto)
    }
}

graph! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    async ListProductsGraph<Context>(limit: i64, offset: i64) -> (products_dto: Result<Json<Vec<Product>>, ApiError>) {
        ProductList(limit, offset) -> products_result >>
        UnwrapResultProducts(products_result) -> products >>
        SerializeProducts(products) -> (products_dto)
    }
}

graph! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    async UpdateProductGraph<Context>(product_id: i64, update: UpdateProduct) -> (product_dto: Result<Json<Product>, ApiError>) {
        ProductUpdate(product_id, update) -> product_result >>
        UnwrapResultOptionProduct(product_result) -> product >>
        SerializeProduct(product) -> (product_dto)
    }
}

graph! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    async DeleteProductGraph<Context>(product_id: i64) -> (result_dto: Result<Json<DeleteResult>, ApiError>) {
        ProductDelete(product_id) -> rows_result >>
        UnwrapResultRowsAffected(rows_result) -> rows >>
        RowsAffectedToDeleteResult(rows) -> result >>
        SerializeDeleteResult(result) -> (result_dto)
    }
}

/*
GetProductData() -> &'a product_input >>
        ValidateProductInputData(&'a product_input) &&
        CheckProductDoesNotExist(&'a product_input) >>
        CreateProduct(*'a product_input) -> &'a product_id >>
        CheckProductCreated(&'a product_id) -> &'a new_product >>
        PublishProductCreatedEvent(&'a product_id) &&
        SendSellerProductCreatedNotification(&'a product_id) &&
        SendAdminProductCreatedNotification(*'a product_id) &&
        GetProductDTO(&'a new_product) -> &'a product_dto
         */
