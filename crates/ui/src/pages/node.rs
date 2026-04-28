use std::sync::Arc;

use askama::Template;

use crate::http::AppHttpError;
use crate::logs::{LogLineView, fetch_node_logs};
use crate::metrics::{fetch_node_metrics, fmt_metric};
use crate::state::AppState;
use crate::traces::{TraceSummaryView, fetch_node_traces};
use crate::util::normalize_symbol;

#[derive(Clone)]
pub(crate) struct MetricCards {
    pub(crate) count: String,
    pub(crate) errors: String,
    pub(crate) success: String,
    pub(crate) fail: String,
    pub(crate) p50: String,
    pub(crate) p95: String,
}

#[derive(Clone)]
pub(crate) struct TestLink {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) target: String,
}

#[derive(Template)]
#[template(path = "pages/node.html")]
pub(crate) struct NodeTemplate {
    pub(crate) title: String,
    pub(crate) active: &'static str,

    pub(crate) label: String,
    pub(crate) target: String,
    pub(crate) ctx_access: String,
    pub(crate) docs: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) deprecated: bool,
    pub(crate) deprecated_reason: Option<String>,

    pub(crate) metrics_graph: String,
    pub(crate) metrics_node: String,
    pub(crate) metrics: MetricCards,

    pub(crate) loki_base_url: String,
    pub(crate) tempo_base_url: String,
    pub(crate) logs: Vec<LogLineView>,
    pub(crate) traces: Vec<TraceSummaryView>,

    pub(crate) tests: Vec<TestLink>,

    pub(crate) raw_schema: Option<String>,
}

pub(crate) async fn node_page_html(
    state: Arc<AppState>,
    node_id: String,
) -> Result<String, AppHttpError> {
    let node = state
        .nodes
        .by_id
        .get(&node_id)
        .ok_or_else(|| AppHttpError::not_found("node not registered"))?;

    let tests = state
        .tests
        .ordered
        .iter()
        .filter(|test| {
            matches!(test.dto.kind, graphium::dto::TestKindDto::Node)
                && test.dto.target_id == node.dto.id
        })
        .map(|test| TestLink {
            id: test.dto.id.clone(),
            name: normalize_symbol(&test.dto.name),
            target: test.dto.target.clone(),
        })
        .collect::<Vec<_>>();

    let metrics_view =
        fetch_node_metrics(&state, &node.dto.metrics_graph, &node.dto.metrics_node).await;
    let metrics = MetricCards {
        count: fmt_metric(metrics_view.count),
        errors: fmt_metric(metrics_view.errors),
        success: fmt_metric(metrics_view.success),
        fail: fmt_metric(metrics_view.fail),
        p50: fmt_metric(metrics_view.p50_seconds),
        p95: fmt_metric(metrics_view.p95_seconds),
    };

    let logs = fetch_node_logs(&state, &node.dto.metrics_graph, &node.dto.metrics_node).await;
    let traces = fetch_node_traces(&state, &node.dto.metrics_graph, &node.dto.metrics_node).await;

    Ok(NodeTemplate {
        title: format!("Node: {} | Graphium UI", node.dto.label),
        active: "dashboard",
        label: node.dto.label.clone(),
        target: node.dto.target.clone(),
        ctx_access: ctx_access_label(node.dto.ctx_access).to_string(),
        docs: node.dto.docs.clone(),
        tags: node.dto.tags.clone(),
        deprecated: node.dto.deprecated,
        deprecated_reason: node.dto.deprecated_reason.clone(),
        metrics_graph: node.dto.metrics_graph.clone(),
        metrics_node: node.dto.metrics_node.clone(),
        metrics,
        loki_base_url: state.loki_base_url.clone(),
        tempo_base_url: state.tempo_base_url.clone(),
        logs,
        traces,
        tests,
        raw_schema: node.dto.raw_schema.clone(),
    }
    .render()
    .expect("render node template"))
}

fn ctx_access_label(access: graphium::dto::CtxAccessDto) -> &'static str {
    match access {
        graphium::dto::CtxAccessDto::None => "none",
        graphium::dto::CtxAccessDto::Ref => "&",
        graphium::dto::CtxAccessDto::Mut => "&mut",
    }
}
