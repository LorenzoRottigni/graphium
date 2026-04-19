use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Form;
use axum::Router;
use serde::Deserialize;

use crate::http::AppHttpError;
use crate::pages::{
    graph as graph_pages, home as home_pages, node as node_pages, tests as tests_pages,
};
use crate::state::{build_state, AppState};
use crate::types::{GraphiumUiConfig, UiError};

pub async fn serve(config: GraphiumUiConfig) -> Result<(), UiError> {
    if config.graphs.is_empty() {
        return Err(UiError::EmptyGraphs);
    }

    let state = Arc::new(build_state(config.prometheus_url, config.graphs));

    let app = Router::new()
        .route("/", get(home))
        .route("/dashboard", get(dashboard))
        .route("/select", get(select_graph_query))
        .route("/select/:id", get(select_graph))
        .route("/graph/:id", get(graph_page))
        .route("/fragment/graph/:id", get(graph_fragment))
        .route("/graph/:id/playground/run", post(run_playground))
        .route("/node/:id", get(node_page))
        .route("/node/:id/playground/run", post(run_node_playground))
        .route("/tests", get(tests_page))
        .route("/tests/run/:id", get(run_test_page))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(UiError::Bind)?;
    axum::serve(listener, app).await.map_err(UiError::Bind)
}

async fn home(State(state): State<Arc<AppState>>) -> Html<String> {
    Html(home_pages::home_page_html(&state))
}

async fn dashboard(State(state): State<Arc<AppState>>) -> Html<String> {
    let default_id = state
        .ordered
        .first()
        .map(|g| g.id.clone())
        .unwrap_or_else(|| "missing".to_string());
    Html(graph_pages::shell_page_html(&state, &default_id))
}

#[derive(Deserialize)]
struct SelectQuery {
    id: String,
}

async fn select_graph_query(
    axum::extract::Query(q): axum::extract::Query<SelectQuery>,
) -> Redirect {
    Redirect::to(&format!("/graph/{}", q.id))
}

async fn select_graph(Path(id): Path<String>) -> Redirect {
    Redirect::to(&format!("/graph/{id}"))
}

async fn graph_page(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, AppHttpError> {
    Ok(Html(graph_pages::shell_page_html(&state, &id)))
}

async fn graph_fragment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response, AppHttpError> {
    let html = graph_pages::render_graph_fragment(state, id.clone(), Default::default()).await?;
    let mut resp = Html(html).into_response();
    let _ = resp.headers_mut().insert(
        axum::http::HeaderName::from_static("hx-push-url"),
        axum::http::HeaderValue::from_str(&format!("/graph/{id}"))
            .unwrap_or_else(|_| axum::http::HeaderValue::from_static("/")),
    );
    Ok(resp)
}

async fn run_playground(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(values): Form<HashMap<String, String>>,
) -> Result<Response, AppHttpError> {
    let graph = state
        .by_id
        .get(&id)
        .ok_or_else(|| AppHttpError::not_found("graph not configured"))?;

    let result = graph
        .playground
        .map(|pg| (pg.run)(&values))
        .unwrap_or_else(|| Err("playground not available for this graph".to_string()));

    let html = graph_pages::render_graph_fragment(
        state,
        id.clone(),
        graph_pages::PlaygroundView {
            values,
            result: Some(result),
        },
    )
    .await?;

    let mut resp = Html(html).into_response();
    let _ = resp.headers_mut().insert(
        axum::http::HeaderName::from_static("hx-push-url"),
        axum::http::HeaderValue::from_str(&format!("/graph/{id}"))
            .unwrap_or_else(|_| axum::http::HeaderValue::from_static("/")),
    );
    Ok(resp)
}

async fn tests_page(State(state): State<Arc<AppState>>) -> Html<String> {
    Html(tests_pages::tests_page_html(&state))
}

async fn node_page(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<node_pages::NodeQuery>,
) -> Result<Html<String>, AppHttpError> {
    Ok(Html(
        node_pages::node_page_html(state, id, query, Default::default()).await?,
    ))
}

async fn run_node_playground(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<node_pages::NodeQuery>,
    Form(values): Form<HashMap<String, String>>,
) -> Result<Response, AppHttpError> {
    let node = state
        .nodes_by_id
        .get(&id)
        .ok_or_else(|| AppHttpError::not_found("node not registered"))?;

    let result = (node.playground_run)(&values);
    let widget = node_pages::NodePlaygroundView {
        values,
        result: Some(result),
    };
    Ok(Html(node_pages::playground_widget_html(
        node,
        query.graph.as_deref(),
        &widget,
    ))
    .into_response())
}

async fn run_test_page(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, AppHttpError> {
    let test = state
        .tests_by_id
        .get(&id)
        .ok_or_else(|| AppHttpError::not_found("test not configured"))?;
    let result = test.run();
    Ok(Html(tests_pages::run_test_page_html(&state, test, &result)))
}
