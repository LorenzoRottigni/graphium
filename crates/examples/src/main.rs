mod graph;
mod node;

use graph::{PropsNestedGraph, RuntimeDataGraph};
use node::Context;

fn main() {
    let mut ctx = Context::default();
    PropsNestedGraph::run(&mut ctx);

    let runtime_graph = RuntimeDataGraph();

    println!(
        "runtime graph '{}' has {} nodes and {} edges",
        runtime_graph.name,
        runtime_graph.nodes.len(),
        runtime_graph.edges.len(),
    );

    let _ = ctx;
}
