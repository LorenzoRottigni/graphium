use graphium_macro::{graph};
use crate::context::Context;
use crate::models::{Product};
use crate::nodes::product_service::*;

graph! {
    #[metadata(
        context = Context,
        async = true,
        inputs = (name: String, price: String),
        outputs = (product: Result<Product, String>)
    )]
    CreateProductGraph {
        GetProductInput(name, price) -> &product_input >>
        ValidateProductInputData(&product_input) &
        CheckProductDoesNotExist(&product_input) >>
        ProductCreate(&product_input) -> product
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