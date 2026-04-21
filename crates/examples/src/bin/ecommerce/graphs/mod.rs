// use graphium_macro::graph;
// 
// graph! {
//     CreateProductGraph {
//         GetProductData() -> &product_input >>
//         ValidateProductInputData(&product_input) &&
//         CheckProductDoesNotExist(&product_input) >>
//         CreateProduct(&product_input) -> &product_id >>
//         CheckProductCreated(&product_id) -> &new_product >>
//         PublishProductCreatedEvent(&product_id) &&
//         SendSellerProductCreatedNotification(&product_id) &&
//         SendAdminProductCreatedNotification(&product_id) &&
//         GetProductDTO(&new_product) -> &product_dto
//     }
// }
