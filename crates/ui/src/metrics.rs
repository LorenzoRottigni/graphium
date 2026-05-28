use reqwest::Url;
use serde::Deserialize;

use crate::state::AppState;
use crate::time_range::TimeRange;

#[derive(Default)]
pub(crate) struct MetricsView {
    pub(crate) count: Option<f64>,
    pub(crate) errors: Option<f64>,
    pub(crate) success: Option<f64>,
    pub(crate) fail: Option<f64>,
    pub(crate) p50_seconds: Option<f64>,
    pub(crate) p95_seconds: Option<f64>,
}

pub(crate) async fn fetch_metrics(state: &AppState, graph_name: &str, range: TimeRange) -> MetricsView {
    // PromQL string values are quoted with `"..."`. We escape embedded quotes
    // for PromQL (not for Rust), so use `\"` in the resulting query string.
    let esc = graph_name.replace('"', "\\\"");
    let lookback_delta = range.prom_lookback_delta();

    let (count_q, errors_q, success_q, fail_q) = match range.promql_range() {
        Some(window) => (
            // `increase()` can return fractional values due to extrapolation. For
            // execution counts we prefer integer-like display.
            format!(
                r#"round(sum(increase(graphium_graph_count_total{{graph="{esc}"}}[{window}])))"#
            ),
            format!(
                r#"round(sum(increase(graphium_graph_errors_total{{graph="{esc}"}}[{window}])))"#
            ),
            format!(
                r#"round(sum(increase(graphium_graph_success_total{{graph="{esc}"}}[{window}])))"#
            ),
            format!(
                r#"round(sum(increase(graphium_graph_fail_total{{graph="{esc}"}}[{window}])))"#
            ),
        ),
        None => (
            format!(r#"sum(graphium_graph_count_total{{graph="{esc}"}})"#),
            format!(r#"sum(graphium_graph_errors_total{{graph="{esc}"}})"#),
            format!(r#"sum(graphium_graph_success_total{{graph="{esc}"}})"#),
            format!(r#"sum(graphium_graph_fail_total{{graph="{esc}"}})"#),
        ),
    };

    let latency_window = range.promql_range().unwrap_or("5m");
    let p50_q = format!(
        r#"histogram_quantile(0.5, sum(rate(graphium_graph_latency_seconds_bucket{{graph="{esc}"}}[{latency_window}])) by (le))"#
    );
    let p95_q = format!(
        r#"histogram_quantile(0.95, sum(rate(graphium_graph_latency_seconds_bucket{{graph="{esc}"}}[{latency_window}])) by (le))"#
    );

    // For counters, treat "no series" as 0 rather than "n/a".
    let count = prometheus_query_scalar(
        &state.client,
        &state.prometheus_base_url,
        &count_q,
        Some(lookback_delta),
    )
        .await
        .or(Some(0.0));
    let errors = prometheus_query_scalar(
        &state.client,
        &state.prometheus_base_url,
        &errors_q,
        Some(lookback_delta),
    )
        .await
        .or(Some(0.0));
    let success = prometheus_query_scalar(
        &state.client,
        &state.prometheus_base_url,
        &success_q,
        Some(lookback_delta),
    )
        .await
        .or(Some(0.0));
    let fail = prometheus_query_scalar(
        &state.client,
        &state.prometheus_base_url,
        &fail_q,
        Some(lookback_delta),
    )
        .await
        .or(Some(0.0));
    let p50_seconds =
        prometheus_query_scalar(&state.client, &state.prometheus_base_url, &p50_q, None).await;
    let p95_seconds =
        prometheus_query_scalar(&state.client, &state.prometheus_base_url, &p95_q, None).await;

    MetricsView {
        count,
        errors,
        success,
        fail,
        p50_seconds,
        p95_seconds,
    }
}

#[derive(Default)]
pub(crate) struct NodeMetricsView {
    pub(crate) count: Option<f64>,
    pub(crate) errors: Option<f64>,
    pub(crate) success: Option<f64>,
    pub(crate) fail: Option<f64>,
    pub(crate) p50_seconds: Option<f64>,
    pub(crate) p95_seconds: Option<f64>,
}

pub(crate) async fn fetch_node_metrics(
    state: &AppState,
    graph_label: &str,
    node_label: &str,
    range: TimeRange,
) -> NodeMetricsView {
    let g = graph_label.replace('"', "\\\"");
    let n = node_label.replace('"', "\\\"");
    let lookback_delta = range.prom_lookback_delta();
    let (count_q, errors_q, success_q, fail_q) = match range.promql_range() {
        Some(window) => (
            format!(
                r#"round(sum(increase(graphium_node_count_total{{graph="{g}",node="{n}"}}[{window}])))"#
            ),
            format!(
                r#"round(sum(increase(graphium_node_errors_total{{graph="{g}",node="{n}"}}[{window}])))"#
            ),
            format!(
                r#"round(sum(increase(graphium_node_success_total{{graph="{g}",node="{n}"}}[{window}])))"#
            ),
            format!(
                r#"round(sum(increase(graphium_node_fail_total{{graph="{g}",node="{n}"}}[{window}])))"#
            ),
        ),
        None => (
            format!(r#"sum(graphium_node_count_total{{graph="{g}",node="{n}"}})"#),
            format!(r#"sum(graphium_node_errors_total{{graph="{g}",node="{n}"}})"#),
            format!(r#"sum(graphium_node_success_total{{graph="{g}",node="{n}"}})"#),
            format!(r#"sum(graphium_node_fail_total{{graph="{g}",node="{n}"}})"#),
        ),
    };

    let latency_window = range.promql_range().unwrap_or("5m");
    let p50_q = format!(
        r#"histogram_quantile(0.5, sum(rate(graphium_node_latency_seconds_bucket{{graph="{g}",node="{n}"}}[{latency_window}])) by (le))"#
    );
    let p95_q = format!(
        r#"histogram_quantile(0.95, sum(rate(graphium_node_latency_seconds_bucket{{graph="{g}",node="{n}"}}[{latency_window}])) by (le))"#
    );

    let count = prometheus_query_scalar(
        &state.client,
        &state.prometheus_base_url,
        &count_q,
        Some(lookback_delta),
    )
        .await
        .or(Some(0.0));
    let errors = prometheus_query_scalar(
        &state.client,
        &state.prometheus_base_url,
        &errors_q,
        Some(lookback_delta),
    )
        .await
        .or(Some(0.0));
    let success = prometheus_query_scalar(
        &state.client,
        &state.prometheus_base_url,
        &success_q,
        Some(lookback_delta),
    )
        .await
        .or(Some(0.0));
    let fail = prometheus_query_scalar(
        &state.client,
        &state.prometheus_base_url,
        &fail_q,
        Some(lookback_delta),
    )
        .await
        .or(Some(0.0));
    let p50_seconds =
        prometheus_query_scalar(&state.client, &state.prometheus_base_url, &p50_q, None).await;
    let p95_seconds =
        prometheus_query_scalar(&state.client, &state.prometheus_base_url, &p95_q, None).await;

    NodeMetricsView {
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
        Some(v) if v.is_finite() => format!("{v:.4}"),
        None => "n/a".to_string(),
        Some(_) => "n/a".to_string(),
    }
}

pub(crate) fn fmt_count_metric(value: Option<f64>) -> String {
    match value {
        Some(v) if v.is_finite() => {
            let rounded = v.round();
            if (v - rounded).abs() < 1e-9 {
                format!("{:.0}", rounded)
            } else {
                format!("{v:.4}")
            }
        }
        None => "n/a".to_string(),
        Some(_) => "n/a".to_string(),
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

async fn prometheus_query_scalar(
    client: &reqwest::Client,
    base: &str,
    query: &str,
    lookback_delta: Option<&str>,
) -> Option<f64> {
    let mut url = Url::parse(base).ok()?;
    url.set_path("/api/v1/query");

    let mut req = client.get(url).query(&[("query", query)]);
    if let Some(delta) = lookback_delta {
        req = req.query(&[("lookback_delta", delta)]);
    }
    let response = req.send().await.ok()?;
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
