use axum::{self, Json, extract::State};
use graphium_macro::node;
use crate::{graphs::CreateProductGraph, state::AppState};

#[axum::debug_handler]
pub async fn create_product(
    State(state): State<AppState>,
    mut multipart: axum::extract::Multipart,
    
) -> Json<crate::models::Product> {
    let name = multipart.next_field().await.unwrap().unwrap().text().await.unwrap();
    let price = multipart.next_field().await.unwrap().unwrap().text().await.unwrap();
    let mut ctx = state.graphium_ctx.lock().await;
    CreateProductGraph::__graphium_run_async(&mut ctx, name, price).await
}

pub fn update_product() -> axum::routing::MethodRouter {
    node! {
        pub async fn update_product() -> String {
            return "updated".into()
        }
    }
    axum::routing::post(update_product)
}