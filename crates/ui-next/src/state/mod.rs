use std::collections::HashMap;

use super::state::{graph::ConfiguredGraph, node::UiNode, test::UiTest};
use graphium::export::{GraphDefDto, GraphStepDto};

pub(crate) mod build;
pub mod graph;
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
    pub(crate) ordered: Vec<ConfiguredGraph>,

    /// Lookup table of graphs by id (canonical “find graph by id” store for routes like `/graph/:id`).
    pub(crate) by_id: HashMap<String, ConfiguredGraph>,

    /// Ordered list of all UI tests (used to render the tests page consistently).
    /// Typically sorted by test name and deduped by test id.
    pub(crate) tests_ordered: Vec<UiTest>,

    /// Lookup table of tests by id (used to run a specific test via `/tests/run/:id`).
    pub(crate) tests_by_id: HashMap<String, UiTest>,

    /// Lookup table of nodes by id (used to render node pages / node-scoped views).
    pub(crate) nodes_by_id: HashMap<String, UiNode>,
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
