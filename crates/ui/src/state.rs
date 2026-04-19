use std::collections::{HashMap, HashSet};

use graphium::export::{GraphDefDto, GraphStepDto};

use crate::types::ConfiguredGraph;
use crate::util::{normalize_symbol, slugify};

#[derive(Clone)]
pub(crate) struct UiNode {
    pub(crate) dto: graphium::export::NodeDto,
}

#[derive(Clone)]
pub(crate) struct UiTest {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) kind: graphium::test_registry::TestKind,
    pub(crate) target: String,
    pub(crate) run: fn() -> Result<(), String>,
}

impl UiTest {
    pub(crate) fn kind_label(&self) -> &'static str {
        match self.kind {
            graphium::test_registry::TestKind::Node => "Node",
            graphium::test_registry::TestKind::Graph => "Graph",
        }
    }

    pub(crate) fn run(&self) -> TestExecution {
        match (self.run)() {
            Ok(()) => TestExecution {
                passed: true,
                message: "ok".to_string(),
            },
            Err(err) => TestExecution {
                passed: false,
                message: err,
            },
        }
    }
}

#[derive(Clone)]
pub(crate) struct TestExecution {
    pub(crate) passed: bool,
    pub(crate) message: String,
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) prometheus_base_url: String,
    pub(crate) client: reqwest::Client,
    pub(crate) ordered: Vec<ConfiguredGraph>,
    pub(crate) by_id: HashMap<String, ConfiguredGraph>,
    pub(crate) tests_ordered: Vec<UiTest>,
    pub(crate) tests_by_id: HashMap<String, UiTest>,
    pub(crate) nodes_by_id: HashMap<String, UiNode>,
}

pub(crate) fn build_state(prometheus_url: String, graphs: Vec<ConfiguredGraph>) -> AppState {
    let mut ordered = graphs;
    let mut by_id = ordered
        .iter()
        .cloned()
        .map(|g| (g.id.clone(), g))
        .collect::<HashMap<_, _>>();

    // Expand the configured root graphs into a fixed set of graphs/nodes
    // exported at build time by `graph!` / `node!`.
    let root_exports: Vec<graphium::export::GraphDto> =
        ordered.iter().map(|g| g.export.clone()).collect();
    let bundle = graphium::export::GraphiumBundleDto::from_graph_roots(&root_exports);
    let graphium::export::GraphiumBundleDto {
        graphs: bundle_graphs,
        nodes: bundle_nodes,
        ..
    } = bundle;

    for export in bundle_graphs {
        if by_id.contains_key(&export.id) {
            continue;
        }
        let candidate = ConfiguredGraph::from_export(export);
        by_id.insert(candidate.id.clone(), candidate.clone());
        ordered.push(candidate);
    }

    let tests_ordered: Vec<UiTest> = graphium::test_registry::registered_tests()
        .into_iter()
        .map(|test| UiTest {
            id: format!(
                "{}-{}-{}",
                match test.kind {
                    graphium::test_registry::TestKind::Node => "node",
                    graphium::test_registry::TestKind::Graph => "graph",
                },
                slugify(test.target),
                slugify(test.name)
            ),
            name: test.name.to_string(),
            kind: test.kind,
            target: test.target.to_string(),
            run: test.run,
        })
        .collect();

    let tests_by_id = tests_ordered
        .iter()
        .cloned()
        .map(|t| (t.id.clone(), t))
        .collect::<HashMap<_, _>>();

    let nodes_by_id = bundle_nodes
        .into_iter()
        .map(|dto| (dto.id.clone(), UiNode { dto }))
        .collect::<HashMap<_, _>>();

    AppState {
        prometheus_base_url: prometheus_url,
        client: reqwest::Client::new(),
        ordered,
        by_id,
        tests_ordered,
        tests_by_id,
        nodes_by_id,
    }
}

pub(crate) fn collect_graph_node_symbols(graph: &GraphDefDto) -> HashSet<String> {
    let mut symbols = HashSet::new();
    collect_graph_node_symbols_from_steps(&graph.steps, &mut symbols);
    symbols
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

fn collect_graph_node_symbols_from_steps(steps: &[GraphStepDto], out: &mut HashSet<String>) {
    for step in steps {
        match step {
            GraphStepDto::Node { name, .. } => {
                out.insert(normalize_symbol(name));
            }
            GraphStepDto::Nested { graph, .. } => {
                collect_graph_node_symbols_from_steps(&graph.steps, out)
            }
            GraphStepDto::Parallel { branches, .. } => {
                for branch in branches {
                    collect_graph_node_symbols_from_steps(branch, out);
                }
            }
            GraphStepDto::Route { cases, .. } => {
                for case in cases {
                    collect_graph_node_symbols_from_steps(&case.steps, out);
                }
            }
            GraphStepDto::While { body, .. } | GraphStepDto::Loop { body, .. } => {
                collect_graph_node_symbols_from_steps(body, out);
            }
            GraphStepDto::Break => {}
        }
    }
}

// Note: nested graph discovery is DTO-driven now via `GraphDto.subgraphs`.
