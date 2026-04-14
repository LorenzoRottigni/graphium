mod graph;
mod node;

use graph::PropsNestedGraph;
use node::Context;

fn main() {
    let mut ctx = Context::default();
    PropsNestedGraph::run(&mut ctx);

    let _ = ctx;
}
