mod graph;
mod node;

use graph::PropsGraph;
use node::Context;

fn main() {
    let mut ctx = Context::default();
    PropsGraph::run(&mut ctx);
}
