use reqwest::Url;
use serde::Deserialize;

use crate::state::AppState;

#[derive(Clone, Debug)]
pub(crate) struct LogLineView {
    pub(crate) ts_unix_nanos: String,
    pub(crate) line: String,
}

pub(crate) async fn fetch_graph_logs(state: &AppState, graph_name: &str) -> Vec<LogLineView> {
    let query = format!(
        "{{service_name=\"graphium\",graph=\"{}\"}}",
        graph_name.replace('"', "\\\"")
    );
    fetch_loki_logs(state, &query, 30).await
}

pub(crate) async fn fetch_node_logs(
    state: &AppState,
    graph_name: &str,
    node_name: &str,
) -> Vec<LogLineView> {
    let query = format!(
        "{{service_name=\"graphium\",graph=\"{}\",node=\"{}\"}}",
        graph_name.replace('"', "\\\""),
        node_name.replace('"', "\\\"")
    );
    fetch_loki_logs(state, &query, 30).await
}

#[derive(Debug, Deserialize)]
struct LokiResponse {
    status: String,
    data: LokiData,
}

#[derive(Debug, Deserialize)]
struct LokiData {
    result: Vec<LokiStream>,
}

#[derive(Debug, Deserialize)]
struct LokiStream {
    values: Vec<(String, String)>,
}

async fn fetch_loki_logs(state: &AppState, query: &str, limit: usize) -> Vec<LogLineView> {
    let mut url = match Url::parse(&state.loki_base_url) {
        Ok(u) => u,
        Err(_) => return Vec::new(),
    };
    url.set_path("/loki/api/v1/query_range");

    // Last 30 minutes.
    let end_ns = unix_now_nanos();
    let start_ns = end_ns.saturating_sub(30 * 60 * 1_000_000_000);

    let response = state
        .client
        .get(url)
        .query(&[
            ("query", query),
            ("limit", &limit.to_string()),
            ("direction", "backward"),
            ("start", &start_ns.to_string()),
            ("end", &end_ns.to_string()),
        ])
        .send()
        .await;

    let Ok(response) = response else {
        return Vec::new();
    };
    if !response.status().is_success() {
        return Vec::new();
    }

    let Ok(payload) = response.json::<LokiResponse>().await else {
        return Vec::new();
    };
    if payload.status != "success" {
        return Vec::new();
    }

    let mut lines = Vec::new();
    for stream in payload.data.result {
        for (ts, line) in stream.values {
            lines.push(LogLineView {
                ts_unix_nanos: ts,
                line,
            });
        }
    }
    // Result is already backward, but we might have multiple streams.
    lines.sort_by(|a, b| b.ts_unix_nanos.cmp(&a.ts_unix_nanos));
    lines.truncate(limit);
    lines
}

fn unix_now_nanos() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let Ok(dur) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    dur.as_nanos().min(u128::from(u64::MAX)) as u64
}
