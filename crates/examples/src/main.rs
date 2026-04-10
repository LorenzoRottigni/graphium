mod graph;
mod node;

use graph::DataGraph2;
use node::Context;

fn main() {
    let mut ctx = Context::default();
    DataGraph2::run(&mut ctx);
}
