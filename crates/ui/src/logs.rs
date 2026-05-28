use reqwest::Url;
use serde::Deserialize;

use crate::state::AppState;
use crate::time_range::TimeRange;

#[derive(Clone, Debug)]
pub(crate) struct LogLineView {
    pub(crate) ts_unix_nanos: String,
    pub(crate) line: String,
}

pub(crate) async fn fetch_graph_logs(
    state: &AppState,
    graph_name: &str,
    range: TimeRange,
) -> Vec<LogLineView> {
    // The OTLP->Loki pipeline ships Graphium fields as JSON log attributes.
    // Query by parsing JSON rather than relying on Loki labels.
    let query = format!(
        "{{service_name=\"{}\"}} | json | attributes_graph=\"{}\"",
        state.service_name.replace('"', "\\\""),
        graph_name.replace('"', "\\\"")
    );
    fetch_loki_logs(state, &query, range, 30).await
}

pub(crate) async fn fetch_node_logs(
    state: &AppState,
    graph_name: &str,
    node_name: &str,
    range: TimeRange,
) -> Vec<LogLineView> {
    let query = format!(
        "{{service_name=\"{}\"}} | json | attributes_graph=\"{}\" | attributes_node=\"{}\"",
        state.service_name.replace('"', "\\\""),
        graph_name.replace('"', "\\\""),
        node_name.replace('"', "\\\"")
    );
    fetch_loki_logs(state, &query, range, 30).await
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

async fn fetch_loki_logs(
    state: &AppState,
    query: &str,
    range: TimeRange,
    limit: usize,
) -> Vec<LogLineView> {
    let mut url = match Url::parse(&state.loki_base_url) {
        Ok(u) => u,
        Err(_) => return Vec::new(),
    };
    url.set_path("/loki/api/v1/query_range");

    let end_ns = unix_now_nanos();
    let primary_start_ns = match range.seconds() {
        Some(window_s) => end_ns.saturating_sub(window_s.saturating_mul(1_000_000_000)),
        // Loki commonly enforces a maximum query range; treat "all time" as
        // "the widest window we can query" (aligned with `.docker/loki.yml`).
        None => end_ns.saturating_sub(365u64.saturating_mul(24 * 60 * 60).saturating_mul(1_000_000_000)),
    };

    let response = loki_query_range(state, &url, query, limit, primary_start_ns, end_ns).await;
    let response = match response {
        Ok(r) if r.status().is_success() => Ok(r),
        Ok(r) => {
            // If "all time" is too wide for Loki's configured query limits,
            // fall back to a 30d window rather than showing an empty box.
            if range == TimeRange::All {
                let window_s = 30u64.saturating_mul(24 * 60 * 60);
                let fallback_start_ns =
                    end_ns.saturating_sub(window_s.saturating_mul(1_000_000_000));
                loki_query_range(state, &url, query, limit, fallback_start_ns, end_ns).await
            } else {
                Ok(r)
            }
        }
        Err(e) => Err(e),
    };

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

async fn loki_query_range(
    state: &AppState,
    url: &Url,
    query: &str,
    limit: usize,
    start_ns: u64,
    end_ns: u64,
) -> Result<reqwest::Response, reqwest::Error> {
    state
        .client
        .get(url.clone())
        .query(&[
            ("query", query),
            ("limit", &limit.to_string()),
            ("direction", "backward"),
            ("start", &start_ns.to_string()),
            ("end", &end_ns.to_string()),
        ])
        .send()
        .await
}
