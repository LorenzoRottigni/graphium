use super::state::{graph::UiGraph, index::UiIndex, node::UiNode, test::UiTest};
use graphium::export::{GraphDefDto, GraphStepDto};

pub(crate) mod build;
pub mod graph;
pub(crate) mod index;
pub(crate) mod node;
pub(crate) mod playground;
pub(crate) mod test;

#[derive(Clone)]
pub(crate) struct AppState {
    /// Base URL for Prometheus (used to build metric queries / links in the UI).
    pub(crate) prometheus_base_url: String,

    /// Shared HTTP client used by handlers to call Prometheus and other endpoints.
    pub(crate) client: reqwest::Client,

    /// Ordered list of graphs used for UI iteration (dropdowns) and default selection.
    /// Starts from configured root graphs, then appends any discovered subgraphs.
    pub(crate) graphs: UiIndex<UiGraph>,

    pub(crate) tests: UiIndex<UiTest>,
    pub(crate) nodes: UiIndex<UiNode>,
}

pub(crate) fn collect_graph_node_names(graph: &GraphDefDto) -> Vec<String> {
    let mut out = Vec::new();
    collect_graph_node_names_from_steps(&graph.steps, &mut out);
    out.sort();
    out.dedup();
    out
}

fn collect_graph_node_names_from_steps(steps: &[GraphStepDto], out: &mut Vec<String>) {
    for step in steps {
        match step {
            GraphStepDto::Node { name, .. } => out.push(name.to_string()),
            GraphStepDto::Nested { graph, .. } => {
                collect_graph_node_names_from_steps(&graph.steps, out)
            }
            GraphStepDto::Parallel { branches, .. } => {
                for branch in branches {
                    collect_graph_node_names_from_steps(branch, out);
                }
            }
            GraphStepDto::Route { cases, .. } => {
                for case in cases {
                    collect_graph_node_names_from_steps(&case.steps, out);
                }
            }
            GraphStepDto::While { body, .. } | GraphStepDto::Loop { body, .. } => {
                collect_graph_node_names_from_steps(body, out);
            }
            GraphStepDto::Break => {}
        }
    }
}
