use axum::{
    self, Json,
    extract::{Path, Query, State},
};

use crate::{
    graphs::{
        CreateProductGraph, DeleteProductGraph, GetProductGraph, ListProductsGraph,
        UpdateProductGraph,
    },
    models::{ListProductsQuery, UpdateProduct},
    state::AppState,
};

#[axum::debug_handler]
pub async fn create_product(
    State(state): State<AppState>,
    mut multipart: axum::extract::Multipart,
) -> Json<crate::models::Product> {
    let name = multipart
        .next_field()
        .await
        .unwrap()
        .unwrap()
        .text()
        .await
        .unwrap();
    let price = multipart
        .next_field()
        .await
        .unwrap()
        .unwrap()
        .text()
        .await
        .unwrap();
    let mut ctx = state.graphium_ctx.lock().await;
    CreateProductGraph::__graphium_run_async(&mut ctx, name, price).await
}

#[axum::debug_handler]
pub async fn get_product(
    State(state): State<AppState>,
    Path(product_id): Path<i64>,
) -> Json<crate::models::Product> {
    let mut ctx = state.graphium_ctx.lock().await;
    GetProductGraph::__graphium_run_async(&mut ctx, product_id).await
}

#[axum::debug_handler]
pub async fn list_products(
    State(state): State<AppState>,
    Query(query): Query<ListProductsQuery>,
) -> Json<Vec<crate::models::Product>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    let mut ctx = state.graphium_ctx.lock().await;
    ListProductsGraph::__graphium_run_async(&mut ctx, limit, offset).await
}

#[axum::debug_handler]
pub async fn update_product(
    State(state): State<AppState>,
    Path(product_id): Path<i64>,
    Json(update): Json<UpdateProduct>,
) -> Json<crate::models::Product> {
    let mut ctx = state.graphium_ctx.lock().await;
    UpdateProductGraph::__graphium_run_async(&mut ctx, product_id, update).await
}

#[axum::debug_handler]
pub async fn delete_product(
    State(state): State<AppState>,
    Path(product_id): Path<i64>,
) -> Json<crate::models::DeleteResult> {
    let mut ctx = state.graphium_ctx.lock().await;
    DeleteProductGraph::__graphium_run_async(&mut ctx, product_id).await
}
