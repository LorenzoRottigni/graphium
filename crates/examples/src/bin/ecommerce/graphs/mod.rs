use crate::context::Context;
use crate::models::{DeleteResult, Product, UpdateProduct};
use crate::nodes::product_service::*;
use axum::Json;
use graphium_macro::graph;

graph! {
    #[metadata(
        context = Context,
        async = true,
        inputs = (name: String, price: String),
        outputs = (product_dto: Json<Product>)
    )]
    CreateProductGraph {
        GetProductInput(name, price) -> &product_input >>
        ValidateProductInputData(&product_input) &
        CheckProductDoesNotExist(&product_input) >>
        ProductCreate(&product_input) -> product >>
        SerializeProduct(product) -> product_dto
    }
}

graph! {
    #[metadata(
        context = Context,
        async = true,
        inputs = (product_id: i64),
        outputs = (product_dto: Json<Product>)
    )]
    GetProductGraph {
        ProductGetById(product_id) -> product_result >>
        UnwrapResultOptionProduct(product_result) -> product >>
        SerializeProduct(product) -> product_dto
    }
}

graph! {
    #[metadata(
        context = Context,
        async = true,
        inputs = (limit: i64, offset: i64),
        outputs = (products_dto: Json<Vec<Product>>)
    )]
    ListProductsGraph {
        ProductList(limit, offset) -> products_result >>
        UnwrapResultProducts(products_result) -> products >>
        SerializeProducts(products) -> products_dto
    }
}

graph! {
    #[metadata(
        context = Context,
        async = true,
        inputs = (product_id: i64, update: UpdateProduct),
        outputs = (product_dto: Json<Product>)
    )]
    UpdateProductGraph {
        ProductUpdate(product_id, update) -> product_result >>
        UnwrapResultOptionProduct(product_result) -> product >>
        SerializeProduct(product) -> product_dto
    }
}

graph! {
    #[metadata(
        context = Context,
        async = true,
        inputs = (product_id: i64),
        outputs = (result_dto: Json<DeleteResult>)
    )]
    DeleteProductGraph {
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
