mod graph;
mod node;

use graph::PropsGraph;
use graphio::Controller;
use node::Context;

fn main() {
    let mut ctx = Context::default();
    Controller::new().run(&PropsGraph, &mut ctx);
}
