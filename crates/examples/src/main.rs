mod graph;
mod node;

use graph::{PropsNestedGraph, RuntimeDataGraph};
use node::Context;

fn main() {
    let mut ctx = Context::default();
    PropsNestedGraph::run(&mut ctx);

    let mut runtime_graph = RuntimeDataGraph();
    runtime_graph.run(&mut ctx);

    println!(
        "runtime graph '{}' has {} nodes, {} edges and {} run(s)",
        runtime_graph.name,
        runtime_graph.nodes.len(),
        runtime_graph.edges.len(),
        runtime_graph.runs()
    );
}
