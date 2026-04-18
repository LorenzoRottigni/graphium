use reqwest::Url;
use serde::Deserialize;

use crate::state::AppState;

#[derive(Default)]
pub(crate) struct MetricsView {
    pub(crate) count: Option<f64>,
    pub(crate) errors: Option<f64>,
    pub(crate) success: Option<f64>,
    pub(crate) fail: Option<f64>,
    pub(crate) p50_seconds: Option<f64>,
    pub(crate) p95_seconds: Option<f64>,
}

pub(crate) async fn fetch_metrics(state: &AppState, graph_name: &str) -> MetricsView {
    let esc = graph_name.replace('"', "\\\"");
    let count_q = format!(r#"sum(graphium_graph_count_total{{graph=\"{esc}\"}})"#);
    let errors_q = format!(r#"sum(graphium_graph_errors_total{{graph=\"{esc}\"}})"#);
    let success_q = format!(r#"sum(graphium_graph_success_total{{graph=\"{esc}\"}})"#);
    let fail_q = format!(r#"sum(graphium_graph_fail_total{{graph=\"{esc}\"}})"#);
    let p50_q = format!(
        r#"histogram_quantile(0.5, sum(rate(graphium_graph_latency_seconds_bucket{{graph=\"{esc}\"}}[5m])) by (le))"#
    );
    let p95_q = format!(
        r#"histogram_quantile(0.95, sum(rate(graphium_graph_latency_seconds_bucket{{graph=\"{esc}\"}}[5m])) by (le))"#
    );

    let count = prometheus_query_scalar(&state.client, &state.prometheus_base_url, &count_q).await;
    let errors =
        prometheus_query_scalar(&state.client, &state.prometheus_base_url, &errors_q).await;
    let success =
        prometheus_query_scalar(&state.client, &state.prometheus_base_url, &success_q).await;
    let fail = prometheus_query_scalar(&state.client, &state.prometheus_base_url, &fail_q).await;
    let p50_seconds =
        prometheus_query_scalar(&state.client, &state.prometheus_base_url, &p50_q).await;
    let p95_seconds =
        prometheus_query_scalar(&state.client, &state.prometheus_base_url, &p95_q).await;

    MetricsView {
        count,
        errors,
        success,
        fail,
        p50_seconds,
        p95_seconds,
    }
}

pub(crate) fn fmt_metric(value: Option<f64>) -> String {
    match value {
        Some(v) => format!("{v:.4}"),
        None => "n/a".to_string(),
    }
}

#[derive(Debug, Deserialize)]
struct PrometheusResponse {
    status: String,
    data: PrometheusData,
}

#[derive(Debug, Deserialize)]
struct PrometheusData {
    result: Vec<PrometheusResult>,
}

#[derive(Debug, Deserialize)]
struct PrometheusResult {
    value: (f64, String),
}

async fn prometheus_query_scalar(client: &reqwest::Client, base: &str, query: &str) -> Option<f64> {
    let mut url = Url::parse(base).ok()?;
    url.set_path("/api/v1/query");

    let response = client
        .get(url)
        .query(&[("query", query)])
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }

    let payload: PrometheusResponse = response.json().await.ok()?;
    if payload.status != "success" {
        return None;
    }

    let value = payload.data.result.first()?.value.1.parse::<f64>().ok()?;
    Some(value)
}
