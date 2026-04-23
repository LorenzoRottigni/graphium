use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use askama::Template;

use crate::http::AppHttpError;
use crate::mermaid::to_mermaid;
use crate::metrics::{fetch_metrics, fmt_metric};
use crate::state::{AppState, collect_graph_node_names, graph::UiGraph};
use crate::util::{normalize_symbol, slugify};

#[derive(Default, Clone)]
pub(crate) struct PlaygroundView {
    pub(crate) values: HashMap<String, String>,
    pub(crate) result: Option<Result<String, String>>,
}

#[derive(Template)]
#[template(path = "pages/dashboard.html")]
pub(crate) struct DashboardTemplate<'a> {
    pub(crate) title: &'a str,
    pub(crate) active: &'a str,
    pub(crate) graphs: &'a [UiGraph],
    pub(crate) selected_id: &'a str,
}

pub(crate) fn dashboard_page_html(state: &AppState, selected_id: &str) -> String {
    DashboardTemplate {
        title: "Dashboard | Graphium UI",
        active: "dashboard",
        graphs: &state.graphs.ordered,
        selected_id,
    }
    .render()
    .expect("render dashboard template")
}

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
pub(crate) struct NodeLink {
    pub(crate) id: String,
    pub(crate) label: String,
}

#[derive(Clone)]
pub(crate) struct TestLink {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) target: String,
}

#[derive(Clone)]
pub(crate) struct PlaygroundResultView {
    pub(crate) ok: bool,
    pub(crate) message: String,
}

#[derive(Clone)]
pub(crate) struct PlaygroundInputView {
    pub(crate) name: String,
    pub(crate) ty: String,
    pub(crate) is_bool: bool,
    pub(crate) checked: bool,
    pub(crate) value: String,
}

#[derive(Clone)]
pub(crate) struct PlaygroundTemplateView {
    pub(crate) supported: bool,
    pub(crate) context: String,
    pub(crate) outputs_label: String,
    pub(crate) has_bool_inputs: bool,
    pub(crate) inputs: Vec<PlaygroundInputView>,
    pub(crate) result: Option<PlaygroundResultView>,
}

#[derive(Template)]
#[template(path = "pages/graph_fragment.html")]
pub(crate) struct GraphFragmentTemplate {
    pub(crate) graph_id: String,
    pub(crate) graph_docs: Option<String>,
    pub(crate) graph_tags: Vec<String>,
    pub(crate) graph_deprecated: bool,
    pub(crate) graph_deprecated_reason: Option<String>,
    pub(crate) mermaid: String,
    pub(crate) prometheus_base_url: String,
    pub(crate) metrics: MetricCards,
    pub(crate) nodes: Vec<NodeLink>,
    pub(crate) raw_schema: String,
    pub(crate) playground: Option<PlaygroundTemplateView>,
    pub(crate) graph_tests: Vec<TestLink>,
    pub(crate) node_tests: Vec<TestLink>,
}

pub(crate) async fn render_graph_fragment(
    state: Arc<AppState>,
    id: String,
    playground_view: PlaygroundView,
) -> Result<String, AppHttpError> {
    let graph = state
        .graphs
        .by_id
        .get(&id)
        .ok_or_else(|| AppHttpError::not_found("graph not configured"))?;

    let linkable_graphs: HashSet<String> = state.graphs.by_id.keys().cloned().collect();
    let mermaid = to_mermaid(
        &graph.export.def,
        graph.playground.map(|p| p.schema.context),
        &linkable_graphs,
    );

    let metrics = fetch_metrics(&state, &graph.export.def.name).await;
    let metrics = MetricCards {
        count: fmt_metric(metrics.count),
        errors: fmt_metric(metrics.errors),
        success: fmt_metric(metrics.success),
        fail: fmt_metric(metrics.fail),
        p50: fmt_metric(metrics.p50_seconds),
        p95: fmt_metric(metrics.p95_seconds),
    };

    let node_names = collect_graph_node_names(&graph.export.def);
    let nodes = node_names
        .iter()
        .map(|name| NodeLink {
            id: slugify(&normalize_symbol(name)),
            label: normalize_symbol(name),
        })
        .collect::<Vec<_>>();
    let node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();

    let graph_tests = state
        .tests
        .ordered
        .iter()
        .filter(|test| {
            matches!(test.dto.kind, graphium::export::TestKindDto::Graph)
                && test.dto.target_id == graph.id
        })
        .map(|test| TestLink {
            id: test.dto.id.clone(),
            name: normalize_symbol(&test.dto.name),
            target: test.dto.target.clone(),
        })
        .collect::<Vec<_>>();

    let node_tests = state
        .tests
        .ordered
        .iter()
        .filter(|test| {
            matches!(test.dto.kind, graphium::export::TestKindDto::Node)
                && node_ids.contains(&test.dto.target_id)
        })
        .map(|test| TestLink {
            id: test.dto.id.clone(),
            name: normalize_symbol(&test.dto.name),
            target: test.dto.target.clone(),
        })
        .collect::<Vec<_>>();

    let raw_schema = graph.export.raw_schema.clone()
        .unwrap_or_else(|| "Raw schema not available for this graph.".to_string());

    let playground = graph.playground.map(|pg| {
        let inputs = pg
            .schema
            .inputs
            .iter()
            .map(|param| {
                let value = playground_view
                    .values
                    .get(param.name)
                    .cloned()
                    .unwrap_or_default();
                let checked = matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "true" | "1" | "yes" | "on"
                );
                PlaygroundInputView {
                    name: param.name.to_string(),
                    ty: param.ty.to_string(),
                    is_bool: param.ty.trim() == "bool",
                    checked,
                    value,
                }
            })
            .collect::<Vec<_>>();

        let outputs_label = if pg.schema.outputs.is_empty() {
            "Outputs: (none)".to_string()
        } else {
            let mut out = String::new();
            for (idx, param) in pg.schema.outputs.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                out.push_str(param.name);
                out.push_str(": ");
                out.push_str(param.ty);
            }
            format!("Outputs: {out}")
        };

        let result = playground_view.result.map(|r| match r {
            Ok(message) => PlaygroundResultView { ok: true, message },
            Err(message) => PlaygroundResultView { ok: false, message },
        });

        PlaygroundTemplateView {
            supported: pg.supported,
            context: pg.schema.context.to_string(),
            outputs_label,
            has_bool_inputs: pg.schema.inputs.iter().any(|p| p.ty.trim() == "bool"),
            inputs,
            result,
        }
    });

    Ok(GraphFragmentTemplate {
        graph_id: id,
        graph_docs: graph.export.docs.clone(),
        graph_tags: graph.export.tags.clone(),
        graph_deprecated: graph.export.deprecated,
        graph_deprecated_reason: graph.export.deprecated_reason.clone(),
        mermaid,
        prometheus_base_url: state.prometheus_base_url.clone(),
        metrics,
        nodes,
        raw_schema,
        playground,
        graph_tests,
        node_tests,
    }
    .render()
    .expect("render graph fragment"))
}
