use reqwest::Url;
use serde::Deserialize;

use crate::state::AppState;

#[derive(Clone, Debug)]
pub(crate) struct TraceSummaryView {
    pub(crate) trace_id: String,
    pub(crate) root_trace_name: String,
    pub(crate) duration_ms: u64,
    pub(crate) api_trace_url: String,
}

pub(crate) async fn fetch_graph_traces(state: &AppState, graph_name: &str) -> Vec<TraceSummaryView> {
    let q = format!(
        r#"{{ .service.name = "graphium" && .graph = "{}" }}"#,
        graph_name.replace('"', "\\\"")
    );
    tempo_search(state, &q, 10).await
}

pub(crate) async fn fetch_node_traces(
    state: &AppState,
    graph_name: &str,
    node_name: &str,
) -> Vec<TraceSummaryView> {
    let q = format!(
        r#"{{ .service.name = "graphium" && .graph = "{}" && .node = "{}" }}"#,
        graph_name.replace('"', "\\\""),
        node_name.replace('"', "\\\"")
    );
    tempo_search(state, &q, 10).await
}

#[derive(Debug, Deserialize)]
struct TempoSearchResponse {
    #[serde(default)]
    traces: Vec<TempoTrace>,
}

#[derive(Debug, Deserialize)]
struct TempoTrace {
    #[serde(rename = "traceID")]
    trace_id: String,
    #[serde(rename = "rootTraceName", default)]
    root_trace_name: String,
    #[serde(rename = "durationMs", default)]
    duration_ms: u64,
}

async fn tempo_search(state: &AppState, q: &str, limit: usize) -> Vec<TraceSummaryView> {
    let mut url = match Url::parse(&state.tempo_base_url) {
        Ok(u) => u,
        Err(_) => return Vec::new(),
    };
    url.set_path("/api/search");

    let response = state
        .client
        .get(url.clone())
        .query(&[("q", q), ("limit", &limit.to_string())])
        .send()
        .await;

    let Ok(response) = response else {
        return Vec::new();
    };
    if !response.status().is_success() {
        return Vec::new();
    }

    let Ok(payload) = response.json::<TempoSearchResponse>().await else {
        return Vec::new();
    };

    payload
        .traces
        .into_iter()
        .take(limit)
        .map(|t| TraceSummaryView {
            api_trace_url: format!("{}/api/v2/traces/{}", state.tempo_base_url, t.trace_id),
            trace_id: t.trace_id,
            root_trace_name: t.root_trace_name,
            duration_ms: t.duration_ms,
        })
        .collect()
}

