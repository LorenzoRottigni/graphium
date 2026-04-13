mod graph;
mod node;

use graph::PropsNestedGraph;
use graphio::Controller;
use node::Context;

fn main() {
    let mut ctx = Context::default();
    Controller::new().run(&PropsNestedGraph, &mut ctx);
}
