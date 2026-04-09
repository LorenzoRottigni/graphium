mod node;
mod graph;

use graphio_macro::controller;
use graph::data_pipeline;

controller! {
    name: MyController,
    graphs: [data_pipeline]
}

fn main() {
    MyController::run();
}