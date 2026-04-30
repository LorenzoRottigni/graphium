use crate::context::Context;
use crate::models::{DeleteResult, Product, UpdateProduct};
use crate::nodes::product_service::*;
use axum::Json;
use graphium::graph;

graph! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    async CreateProductGraph<'a, Context>(name: String, price: String) -> (product_dto: Json<Product>) {
        GetProductInput(name, price) -> &'a product_input >>
        ValidateProductInputData(&'a product_input) &&
        CheckProductDoesNotExist(&'a product_input) >>
        ProductCreate(&'a product_input) -> product >>
        SerializeProduct(product) -> product_dto
    }
}

graph! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    async GetProductGraph<'a, Context>(product_id: i64) -> (product_dto: Json<Product>) {
        ProductGetById(product_id) -> product_result >>
        UnwrapResultOptionProduct(product_result) -> product >>
        SerializeProduct(product) -> product_dto
    }
}

graph! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    async ListProductsGraph<'a, Context>(limit: i64, offset: i64) -> (products_dto: Json<Vec<Product>>) {
        ProductList(limit, offset) -> products_result >>
        UnwrapResultProducts(products_result) -> products >>
        SerializeProducts(products) -> products_dto
    }
}

graph! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    async UpdateProductGraph<'a, Context>(product_id: i64, update: UpdateProduct) -> (product_dto: Json<Product>) {
        ProductUpdate(product_id, update) -> product_result >>
        UnwrapResultOptionProduct(product_result) -> product >>
        SerializeProduct(product) -> product_dto
    }
}

graph! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    async DeleteProductGraph<'a, Context>(product_id: i64) -> (result_dto: Json<DeleteResult>) {
        ProductDelete(product_id) -> rows_result >>
        UnwrapResultRowsAffected(rows_result) -> rows >>
        RowsAffectedToDeleteResult(rows) -> result >>
        SerializeDeleteResult(result) -> result_dto
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
