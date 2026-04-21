use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Form;
use axum::Router;
use serde::Deserialize;
use tower_http::services::ServeDir;

#[derive(Deserialize)]
pub(crate) struct ListQuery {
    pub(crate) page: Option<usize>,
    pub(crate) sort: Option<String>,
    pub(crate) search: Option<String>,
}

use crate::{
    config::GraphiumUiConfig,
    error::UiError,
    http::AppHttpError,
    pages::{graph as graph_pages, graphs as graphs_pages, home as home_pages, node as node_pages, nodes as nodes_pages, tests as tests_pages},
    state::{build::build_state, AppState},
};

pub async fn serve(config: GraphiumUiConfig) -> Result<(), UiError> {
    if config.graphs.is_empty() {
        return Err(UiError::EmptyGraphs);
    }

    let state = Arc::new(build_state(config.prometheus_url, config.graphs));

    let app = Router::new()
        .route("/", get(home))
        .route("/dashboard", get(dashboard))
        .route("/graphs", get(graphs))
        .route("/nodes", get(nodes))
        .route("/select", get(select_graph_query))
        .route("/select/:id", get(select_graph))
        .route("/graph/:id", get(graph_page))
        .route("/fragment/graph/:id", get(graph_fragment))
        .route("/graph/:id/playground/run", post(run_playground))
        .route("/node/:id", get(node_page))
        .route("/tests", get(tests_page))
        .route("/tests/run/:id", get(run_test_page).post(run_test_execute))
        .with_state(state)
        .nest_service("/assets", ServeDir::new("./crates/ui-next/assets"));

    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(UiError::Bind)?;
    axum::serve(listener, app).await.map_err(UiError::Bind)
}

async fn home(State(state): State<Arc<AppState>>) -> Html<String> {
    home_pages::home(State(state)).await
}

async fn dashboard(State(state): State<Arc<AppState>>) -> Html<String> {
    let default_id = state
        .graphs
        .ordered
        .first()
        .map(|g| g.id.clone())
        .unwrap_or_else(|| "missing".to_string());
    Html(graph_pages::dashboard_page_html(&state, &default_id))
}

async fn graphs(axum::extract::Query(query): axum::extract::Query<ListQuery>, State(state): State<Arc<AppState>>) -> Html<String> {
    Html(graphs_pages::graphs_page_html(&state, query))
}

async fn nodes(axum::extract::Query(query): axum::extract::Query<ListQuery>, State(state): State<Arc<AppState>>) -> Html<String> {
    Html(nodes_pages::nodes_page_html(&state, query))
}

#[derive(Deserialize)]
struct SelectQuery {
    id: String,
}

async fn select_graph_query(axum::extract::Query(q): axum::extract::Query<SelectQuery>) -> Redirect {
    Redirect::to(&format!("/graph/{}", q.id))
}

async fn select_graph(Path(id): Path<String>) -> Redirect {
    Redirect::to(&format!("/graph/{id}"))
}

async fn graph_page(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, AppHttpError> {
    Ok(Html(graph_pages::dashboard_page_html(&state, &id)))
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
        .graphs
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

async fn node_page(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, AppHttpError> {
    Ok(Html(node_pages::node_page_html(state, id).await?))
}

async fn tests_page(axum::extract::Query(query): axum::extract::Query<ListQuery>, State(state): State<Arc<AppState>>) -> Html<String> {
    Html(tests_pages::tests_page_html(&state, query))
}

async fn run_test_page(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, AppHttpError> {
    let test = state
        .tests
        .by_id
        .get(&id)
        .ok_or_else(|| AppHttpError::not_found("test not configured"))?;
    if test.schema.params.is_empty() {
        let result = test.run();
        Ok(Html(tests_pages::run_test_page_html(
            test,
            &test.default_values,
            Some(&result),
        )))
    } else {
        Ok(Html(tests_pages::run_test_page_html(
            test,
            &test.default_values,
            None,
        )))
    }
}

async fn run_test_execute(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(values): Form<HashMap<String, String>>,
) -> Result<Html<String>, AppHttpError> {
    let test = state
        .tests
        .by_id
        .get(&id)
        .ok_or_else(|| AppHttpError::not_found("test not configured"))?;
    let result = test.run_with(&values);
    Ok(Html(tests_pages::run_test_page_html(
        test,
        &values,
        Some(&result),
    )))
}
