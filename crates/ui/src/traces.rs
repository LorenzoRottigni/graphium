use reqwest::Url;
use serde::Deserialize;

use crate::state::AppState;
use crate::time_range::TimeRange;

#[derive(Clone, Debug)]
pub(crate) struct TraceSummaryView {
    pub(crate) trace_id: String,
    pub(crate) root_trace_name: String,
    pub(crate) duration_ms: u64,
    pub(crate) api_trace_url: String,
}

pub(crate) async fn fetch_graph_traces(
    state: &AppState,
    graph_name: &str,
    range: TimeRange,
) -> Vec<TraceSummaryView> {
    let q = format!(
        r#"{{ .service.name = "{}" && .graph = "{}" }}"#,
        state.service_name.replace('"', "\\\""),
        graph_name.replace('"', "\\\"")
    );
    tempo_search(state, &q, range, 10).await
}

pub(crate) async fn fetch_node_traces(
    state: &AppState,
    graph_name: &str,
    node_name: &str,
    range: TimeRange,
) -> Vec<TraceSummaryView> {
    let q = format!(
        r#"{{ .service.name = "{}" && .graph = "{}" && .node = "{}" }}"#,
        state.service_name.replace('"', "\\\""),
        graph_name.replace('"', "\\\""),
        node_name.replace('"', "\\\"")
    );
    tempo_search(state, &q, range, 10).await
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

async fn tempo_search(
    state: &AppState,
    q: &str,
    range: TimeRange,
    limit: usize,
) -> Vec<TraceSummaryView> {
    let mut url = match Url::parse(&state.tempo_base_url) {
        Ok(u) => u,
        Err(_) => return Vec::new(),
    };
    url.set_path("/api/search");

    let now_s = TimeRange::unix_now_seconds();
    let start_s = match range {
        // Tempo enforces a maximum search range. Treat "all time" as "the widest
        // range we can reasonably query" (aligned with `.docker/tempo.yml`).
        TimeRange::All => Some(now_s.saturating_sub(365 * 24 * 60 * 60)),
        _ => range.seconds().map(|window| now_s.saturating_sub(window)),
    };

    let mut params: Vec<(&str, String)> =
        vec![("q", q.to_string()), ("limit", limit.to_string())];
    if let Some(start_s) = start_s {
        params.push(("start", start_s.to_string()));
        params.push(("end", now_s.to_string()));
    }

    let response = state
        .client
        .get(url.clone())
        .query(&params)
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
