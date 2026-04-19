use std::collections::{HashMap, HashSet};

use graphium::{GraphDef, GraphStep};

use crate::types::ConfiguredGraph;
use crate::util::{normalize_symbol, slugify};

#[derive(Clone)]
pub(crate) struct UiNode {
    pub(crate) id: String,
    pub(crate) target: String,
    pub(crate) label: String,
    pub(crate) file: String,
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
    pub(crate) ctx_access: graphium::CtxAccess,
    pub(crate) metrics_graph: String,
    pub(crate) metrics_node: String,
    pub(crate) playground_supported: bool,
    pub(crate) playground_schema: graphium::PlaygroundSchema,
    pub(crate) playground_run: fn(&HashMap<String, String>) -> Result<String, String>,
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

    // Auto-register nested graphs so users can click into subgraphs.
    let mut discovered = HashMap::<String, GraphDef>::new();
    let mut visited = HashSet::<String>::new();
    for graph in &ordered {
        collect_nested_graph_defs(&graph.def, &mut discovered, &mut visited);
    }
    let mut discovered_defs: Vec<GraphDef> = discovered.into_values().collect();
    discovered_defs.sort_by_key(|def| def.name.to_string());
    for def in discovered_defs {
        let candidate = ConfiguredGraph::from_graph_def(def);
        if by_id.contains_key(&candidate.id) {
            continue;
        }
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

    let mut nodes_ordered: Vec<UiNode> = graphium::registered_nodes()
        .into_iter()
        .map(|node| UiNode {
            id: node.id.to_string(),
            target: node.target.to_string(),
            label: node.label.to_string(),
            file: node.file.to_string(),
            start_line: node.start_line,
            end_line: node.end_line,
            ctx_access: node.ctx_access,
            metrics_graph: node.metrics_graph.to_string(),
            metrics_node: node.metrics_node.to_string(),
            playground_supported: node.playground_supported,
            playground_schema: node.playground_schema,
            playground_run: node.playground_run,
        })
        .collect();
    nodes_ordered.sort_by_key(|n| n.label.to_string());
    let nodes_by_id = nodes_ordered
        .iter()
        .cloned()
        .map(|n| (n.id.clone(), n))
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

pub(crate) fn collect_graph_node_symbols(graph: &GraphDef) -> HashSet<String> {
    let mut symbols = HashSet::new();
    collect_graph_node_symbols_from_steps(&graph.steps, &mut symbols);
    symbols
}

pub(crate) fn collect_graph_node_names(graph: &GraphDef) -> Vec<String> {
    let mut out = Vec::new();
    collect_graph_node_names_from_steps(&graph.steps, &mut out);
    out.sort();
    out.dedup();
    out
}

fn collect_graph_node_names_from_steps(steps: &[GraphStep], out: &mut Vec<String>) {
    for step in steps {
        match step {
            GraphStep::Node { name, .. } => out.push(name.to_string()),
            GraphStep::Nested { graph, .. } => {
                collect_graph_node_names_from_steps(&graph.steps, out)
            }
            GraphStep::Parallel { branches, .. } => {
                for branch in branches {
                    collect_graph_node_names_from_steps(branch, out);
                }
            }
            GraphStep::Route { cases, .. } => {
                for case in cases {
                    collect_graph_node_names_from_steps(&case.steps, out);
                }
            }
            GraphStep::While { body, .. } | GraphStep::Loop { body, .. } => {
                collect_graph_node_names_from_steps(body, out);
            }
            GraphStep::Break => {}
        }
    }
}

fn collect_graph_node_symbols_from_steps(steps: &[GraphStep], out: &mut HashSet<String>) {
    for step in steps {
        match step {
            GraphStep::Node { name, .. } => {
                out.insert(normalize_symbol(name));
            }
            GraphStep::Nested { graph, .. } => {
                collect_graph_node_symbols_from_steps(&graph.steps, out)
            }
            GraphStep::Parallel { branches, .. } => {
                for branch in branches {
                    collect_graph_node_symbols_from_steps(branch, out);
                }
            }
            GraphStep::Route { cases, .. } => {
                for case in cases {
                    collect_graph_node_symbols_from_steps(&case.steps, out);
                }
            }
            GraphStep::While { body, .. } | GraphStep::Loop { body, .. } => {
                collect_graph_node_symbols_from_steps(body, out);
            }
            GraphStep::Break => {}
        }
    }
}

fn collect_nested_graph_defs(
    graph: &GraphDef,
    out: &mut HashMap<String, GraphDef>,
    visited: &mut HashSet<String>,
) {
    let id = slugify(graph.name);
    if !visited.insert(id) {
        return;
    }
    collect_nested_graph_defs_from_steps(&graph.steps, out, visited);
}

fn collect_nested_graph_defs_from_steps(
    steps: &[GraphStep],
    out: &mut HashMap<String, GraphDef>,
    visited: &mut HashSet<String>,
) {
    for step in steps {
        match step {
            GraphStep::Nested { graph, .. } => {
                let def = (**graph).clone();
                let id = slugify(def.name);
                out.entry(id).or_insert_with(|| def.clone());
                collect_nested_graph_defs(&def, out, visited);
            }
            GraphStep::Parallel { branches, .. } => {
                for branch in branches {
                    collect_nested_graph_defs_from_steps(branch, out, visited);
                }
            }
            GraphStep::Route { cases, .. } => {
                for case in cases {
                    collect_nested_graph_defs_from_steps(&case.steps, out, visited);
                }
            }
            GraphStep::While { body, .. } | GraphStep::Loop { body, .. } => {
                collect_nested_graph_defs_from_steps(body, out, visited);
            }
            GraphStep::Node { .. } | GraphStep::Break => {}
        }
    }
}
