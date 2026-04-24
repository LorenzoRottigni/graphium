use crate::context::Context;
use crate::models::{DeleteResult, Product, UpdateProduct};
use crate::nodes::product_service::*;
use axum::Json;
use graphium_macro::graph;

graph! {
    async CreateProductGraph<Context>(name: String, price: String) -> (product_dto: Json<Product>) {
        GetProductInput(name, price) -> &product_input >>
        ValidateProductInputData(&product_input) &&
        CheckProductDoesNotExist(&product_input) >>
        ProductCreate(&product_input) -> product >>
        SerializeProduct(product) -> product_dto
    }
}

graph! {
    async GetProductGraph<Context>(product_id: i64) -> (product_dto: Json<Product>) {
        ProductGetById(product_id) -> product_result >>
        UnwrapResultOptionProduct(product_result) -> product >>
        SerializeProduct(product) -> product_dto
    }
}

graph! {
    async ListProductsGraph<Context>(limit: i64, offset: i64) -> (products_dto: Json<Vec<Product>>) {
        ProductList(limit, offset) -> products_result >>
        UnwrapResultProducts(products_result) -> products >>
        SerializeProducts(products) -> products_dto
    }
}

graph! {
    async UpdateProductGraph<Context>(product_id: i64, update: UpdateProduct) -> (product_dto: Json<Product>) {
        ProductUpdate(product_id, update) -> product_result >>
        UnwrapResultOptionProduct(product_result) -> product >>
        SerializeProduct(product) -> product_dto
    }
}

graph! {
    async DeleteProductGraph<Context>(product_id: i64) -> (result_dto: Json<DeleteResult>) {
        ProductDelete(product_id) -> rows_result >>
        UnwrapResultRowsAffected(rows_result) -> rows >>
        RowsAffectedToDeleteResult(rows) -> result >>
        SerializeDeleteResult(result) -> result_dto
    }
}

/*
GetProductData() -> &product_input >>
        ValidateProductInputData(&product_input) &&
        CheckProductDoesNotExist(&product_input) >>
        CreateProduct(*product_input) -> &product_id >>
        CheckProductCreated(&product_id) -> &new_product >>
        PublishProductCreatedEvent(&product_id) &&
        SendSellerProductCreatedNotification(&product_id) &&
        SendAdminProductCreatedNotification(*product_id) &&
        GetProductDTO(&new_product) -> &product_dto
         */
