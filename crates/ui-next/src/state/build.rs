use crate::state::{
    graph::UiGraph,
    index::UiIndex,
    node::UiNode,
    test::UiTest,
    AppState,
};

pub(crate) fn build_state(prometheus_url: String, graphs: Vec<UiGraph>) -> AppState {
    let mut graphs = UiIndex::from_ordered(graphs, |g| &g.id);

    // Expand the configured root graphs into a fixed set of graphs/nodes
    // exported at build time by `graph!` / `node!`.
    let root_exports: Vec<graphium::export::GraphDto> =
        graphs.ordered.iter().map(|g| g.export.clone()).collect();
    let bundle = graphium::export::GraphiumBundleDto::from_graph_roots(&root_exports);

    for export in bundle.graphs.iter() {
        if graphs.by_id.contains_key(&export.id) {
            continue;
        }
        let candidate = UiGraph::from_export(export.clone());
        graphs.insert(candidate.id.clone(), candidate);
    }

    let mut tests_ordered: Vec<UiTest> = graphs
        .ordered
        .iter()
        .flat_map(|g| {
            g.tests.clone().into_iter().map(move |test| UiTest {
                dto: test.dto,
                schema: test.schema,
                default_values: test.default_values,
                run: test.run,
                graph_name: g.name.clone(),
                graph_id: g.id.clone(),
            })
        })
        .collect();
    tests_ordered.sort_by_key(|t| t.dto.name.to_string());

    // Dedupe tests by id in case multiple root graphs reference the same subgraphs/nodes.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    tests_ordered.retain(|t| seen.insert(t.dto.id.clone()));

    let mut nodes_ordered: Vec<UiNode> = bundle.graphs.iter().flat_map(|g| g.nodes.iter().map(|n| UiNode { dto: n.clone(), graph_id: g.id.clone() })).collect();
    nodes_ordered.sort_by_key(|n| n.dto.label.to_string());
    let nodes = UiIndex::from_ordered(nodes_ordered, |n| &n.dto.id);

    AppState {
        prometheus_base_url: prometheus_url,
        client: reqwest::Client::new(),
        graphs,
        tests: UiIndex::from_ordered(tests_ordered, |t| &t.dto.id),
        nodes,
    }
}
