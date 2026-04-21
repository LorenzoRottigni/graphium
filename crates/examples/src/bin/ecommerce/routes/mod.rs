use axum;
use graphium_macro::node;

pub fn create_product() -> axum::routing::MethodRouter {
    use super::nodes::create_product;
    axum::routing::post(create_product)
}

pub fn  update_product() -> axum::routing::MethodRouter {
    node! {
        pub async fn update_product() -> String {
            return "updated".into()
        }
    }
    axum::routing::post(update_product)
}