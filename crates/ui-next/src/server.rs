use axum::{routing::get, Router};
use std::sync::Arc;

use crate::{
    config::GraphiumUiConfig, error::UiError, pages::home::home, state::build::build_state,
};

pub async fn serve(config: GraphiumUiConfig) -> Result<(), UiError> {
    if config.graphs.is_empty() {
        return Err(UiError::EmptyGraphs);
    }

    let state = Arc::new(build_state(config.prometheus_url, config.graphs));

    let app = Router::new().route("/", get(home)).with_state(state);

    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(UiError::Bind)?;
    axum::serve(listener, app).await.map_err(UiError::Bind)
}
