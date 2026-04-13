mod graph;
mod node;

use graph::{PropsNestedGraph, RuntimeDataGraph};
use graphio::{RuntimeArtifacts, RuntimeController};
use node::Context;

fn main() {
    let mut ctx = Context::default();
    PropsNestedGraph::run(&mut ctx);

    let mut runtime_graph = RuntimeDataGraph();
    let controller = RuntimeController;
    let runtime_outputs = controller.execute(&mut runtime_graph, &mut ctx, RuntimeArtifacts::new());

    println!(
        "runtime graph '{}' has {} nodes and {} edges (runs: {}, outputs: {})",
        runtime_graph.name,
        runtime_graph.nodes.len(),
        runtime_graph.edges.len(),
        runtime_graph.state.runs,
        runtime_outputs.len(),
    );

    let _ = ctx;
}
