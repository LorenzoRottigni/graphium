mod node;
mod graph;

use graph::DataGraph;
use node::Context;

fn main() {
    let mut ctx = Context::default();
    DataGraph::run(&mut ctx);
}