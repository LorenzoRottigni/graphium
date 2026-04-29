use axum::http::StatusCode;
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
) -> Result<Json<crate::models::Product>, (StatusCode, String)> {
    let mut name: Option<String> = None;
    let mut price: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid multipart body: {e}"),
        )
    })? {
        let field_name = field.name().map(|n| n.to_string());
        let value = field.text().await.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("invalid multipart field: {e}"),
            )
        })?;

        match field_name.as_deref() {
            Some("name") => name = Some(value),
            Some("price") => price = Some(value),
            // Be lenient with clients that don't send field names: treat first two values
            // as (name, price).
            _ => {
                if name.is_none() {
                    name = Some(value);
                } else if price.is_none() {
                    price = Some(value);
                }
            }
        }
    }

    let name = name.ok_or((
        StatusCode::BAD_REQUEST,
        "missing multipart field `name`".to_string(),
    ))?;
    let price = price.ok_or((
        StatusCode::BAD_REQUEST,
        "missing multipart field `price`".to_string(),
    ))?;

    let mut ctx = state.graphium_ctx.lock().await;
    Ok(CreateProductGraph::run_async(&mut ctx, name, price).await)
}

#[axum::debug_handler]
pub async fn get_product(
    State(state): State<AppState>,
    Path(product_id): Path<i64>,
) -> Json<crate::models::Product> {
    let mut ctx = state.graphium_ctx.lock().await;
    GetProductGraph::run_async(&mut ctx, product_id).await
}

#[axum::debug_handler]
pub async fn list_products(
    State(state): State<AppState>,
    Query(query): Query<ListProductsQuery>,
) -> Json<Vec<crate::models::Product>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    let mut ctx = state.graphium_ctx.lock().await;
    ListProductsGraph::run_async(&mut ctx, limit, offset).await
}

#[axum::debug_handler]
pub async fn update_product(
    State(state): State<AppState>,
    Path(product_id): Path<i64>,
    Json(update): Json<UpdateProduct>,
) -> Json<crate::models::Product> {
    let mut ctx = state.graphium_ctx.lock().await;
    UpdateProductGraph::run_async(&mut ctx, product_id, update).await
}

#[axum::debug_handler]
pub async fn delete_product(
    State(state): State<AppState>,
    Path(product_id): Path<i64>,
) -> Json<crate::models::DeleteResult> {
    let mut ctx = state.graphium_ctx.lock().await;
    DeleteProductGraph::run_async(&mut ctx, product_id).await
}
