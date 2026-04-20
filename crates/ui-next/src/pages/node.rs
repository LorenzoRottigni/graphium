use std::fmt::Write as _;
use std::sync::Arc;

use askama::Template;

use crate::http::AppHttpError;
use crate::metrics::{fetch_node_metrics, fmt_metric};
use crate::state::AppState;

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

    pub(crate) metrics_graph: String,
    pub(crate) metrics_node: String,
    pub(crate) metrics: MetricCards,

    pub(crate) tests: Vec<TestLink>,

    pub(crate) source_available: bool,
    pub(crate) source_file: String,
    pub(crate) source_start: u32,
    pub(crate) source_end: u32,
    pub(crate) source_text: String,
}

pub(crate) async fn node_page_html(
    state: Arc<AppState>,
    node_id: String,
) -> Result<String, AppHttpError> {
    let node = state
        .nodes_by_id
        .get(&node_id)
        .ok_or_else(|| AppHttpError::not_found("node not registered"))?;

    let tests = state
        .tests_ordered
        .iter()
        .filter(|test| {
            matches!(test.dto.kind, graphium::export::TestKindDto::Node)
                && test.dto.target_id == node.dto.id
        })
        .map(|test| TestLink {
            id: test.dto.id.clone(),
            name: test.dto.name.clone(),
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

    let snippet = read_source_snippet(node.dto.source.as_ref());

    Ok(NodeTemplate {
        title: format!("Node: {} | Graphium UI", node.dto.label),
        active: "dashboard",
        label: node.dto.label.clone(),
        target: node.dto.target.clone(),
        ctx_access: ctx_access_label(node.dto.ctx_access).to_string(),
        metrics_graph: node.dto.metrics_graph.clone(),
        metrics_node: node.dto.metrics_node.clone(),
        metrics,
        tests,
        source_available: snippet.available,
        source_file: snippet.file,
        source_start: snippet.start_line,
        source_end: snippet.end_line,
        source_text: snippet.text,
    }
    .render()
    .expect("render node template"))
}

fn ctx_access_label(access: graphium::export::CtxAccessDto) -> &'static str {
    match access {
        graphium::export::CtxAccessDto::None => "none",
        graphium::export::CtxAccessDto::Ref => "&",
        graphium::export::CtxAccessDto::Mut => "&mut",
    }
}

struct SourceSnippet {
    available: bool,
    file: String,
    start_line: u32,
    end_line: u32,
    text: String,
}

fn read_source_snippet(span: Option<&graphium::export::SourceSpanDto>) -> SourceSnippet {
    let Some(span) = span else {
        return SourceSnippet {
            available: false,
            file: String::new(),
            start_line: 0,
            end_line: 0,
            text: String::new(),
        };
    };
    if span.start_line == 0 || span.end_line == 0 || span.end_line < span.start_line {
        return SourceSnippet {
            available: false,
            file: span.file.clone(),
            start_line: span.start_line,
            end_line: span.end_line,
            text: String::new(),
        };
    }

    let Ok(src) = std::fs::read_to_string(&span.file) else {
        return SourceSnippet {
            available: false,
            file: span.file.clone(),
            start_line: span.start_line,
            end_line: span.end_line,
            text: String::new(),
        };
    };

    let mut out = String::new();
    for (idx, line) in src.lines().enumerate() {
        let line_no = (idx + 1) as u32;
        if line_no < span.start_line {
            continue;
        }
        if line_no > span.end_line {
            break;
        }
        let _ = writeln!(out, "{line}");
    }

    SourceSnippet {
        available: !out.is_empty(),
        file: span.file.clone(),
        start_line: span.start_line,
        end_line: span.end_line,
        text: out,
    }
}

