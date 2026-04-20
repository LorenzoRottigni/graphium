use std::collections::HashMap;

use crate::state::{graph::ConfiguredGraph, node::UiNode, test::UiTest, AppState};

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

    let mut tests_ordered: Vec<UiTest> = ordered
        .iter()
        .flat_map(|g| g.tests.clone())
        .map(|test| UiTest {
            dto: test.dto,
            schema: test.schema,
            default_values: test.default_values,
            run: test.run,
        })
        .collect();
    tests_ordered.sort_by_key(|t| t.dto.name.to_string());

    // Dedupe tests by id in case multiple root graphs reference the same subgraphs/nodes.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    tests_ordered.retain(|t| seen.insert(t.dto.id.clone()));

    let tests_by_id = tests_ordered
        .iter()
        .cloned()
        .map(|t| (t.dto.id.clone(), t))
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
