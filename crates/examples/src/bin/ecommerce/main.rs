use std::sync::Arc;

use axum::Router;
use tokio::sync::Mutex;

pub mod context;
pub mod state;
pub mod nodes;
pub mod graphs;
pub mod routes;
pub mod models;

#[tokio::main]
pub async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("bind address");
    let ctx = context::Context::new().await;
    let state = state::AppState { graphium_ctx: Arc::new(Mutex::new(ctx)) };

    let router = Router::new()
        .route(
            "/product/create",
             axum::routing::post(
                routes::product_controller::create_product
            )
        )
        .with_state(state);

    axum::serve(listener, router)
        .await
        .expect("serve failed");
}
