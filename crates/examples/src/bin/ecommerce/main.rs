use std::sync::Arc;

use axum::Router;
use tokio::sync::Mutex;

pub mod context;
pub mod graphs;
pub mod models;
pub mod nodes;
pub mod routes;
pub mod state;

#[tokio::main]
pub async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("bind address");
    let ctx = context::Context::new().await;
    let state = state::AppState {
        graphium_ctx: Arc::new(Mutex::new(ctx)),
    };

    {
        let mut ctx = state.graphium_ctx.lock().await;
        crate::nodes::product_service::MigrateProductsTable::run_async(&mut ctx)
            .await
            .expect("migrate products table");
    }

    let router = Router::new()
        .route(
            "/product/create",
            axum::routing::post(routes::product_controller::create_product),
        )
        .route(
            "/product",
            axum::routing::get(routes::product_controller::list_products),
        )
        .route(
            "/product/:id",
            axum::routing::get(routes::product_controller::get_product),
        )
        .route(
            "/product/:id",
            axum::routing::put(routes::product_controller::update_product),
        )
        .route(
            "/product/:id",
            axum::routing::delete(routes::product_controller::delete_product),
        )
        .with_state(state);

    axum::serve(listener, router).await.expect("serve failed");
}
