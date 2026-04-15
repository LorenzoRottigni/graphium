use graphium::Visualizer;
use graphium_macro::{graph, node};

#[derive(Default)]
struct Context;

node! {
    fn get_number() -> u32 {
        0
    }
}

node! {
    fn duplicate(value: u32) -> (u32, u32) {
        (value, value)
    }
}

node! {
    fn pipe_number(value: u32) -> u32 {
        value
    }
}

node! {
    fn inner_start(a_split: u32, b_split: u32) -> (u32, u32) {
        (a_split, b_split)
    }
}

node! {
    fn inner_finish(a_split: u32, b_split: u32) -> (u32, u32) {
        (a_split, b_split)
    }
}

node! {
    fn left_branch(value: u32) -> u32 {
        value
    }
}

node! {
    fn right_branch(value: u32) -> u32 {
        value
    }
}

#[derive(Clone, Copy)]
enum Status {
    Success,
    Fail,
}

graph! {
    #[metadata(
        context = Context,
        inputs = (a_split: u32, b_split: u32),
        outputs = (a_split: u32, b_split: u32)
    )]
    InnerGraph {
        InnerStart(a_split, b_split) -> (a_split, b_split) >>
        InnerFinish(a_split, b_split) -> (a_split, b_split)
    }
}

graph! {
    #[metadata(context = Context)]
    OwnedGraph {
        GetNumber() -> (a_number) >>
        Duplicate(a_number) -> (a_split, b_split) >>
        LeftBranch(a_split) -> (a_split) & RightBranch(b_split) -> (b_split) >>
        InnerGraph::run(a_split, b_split) -> (a_split, b_split) >>
        @match Status::Success -> (a_split) {
            Status::Success => PipeNumber(a_split) -> (a_split),
            Status::Fail => PipeNumber(b_split) -> (a_split),
        } >>
        PipeNumber(a_split) -> (a_split)
    }
}

fn main() {
    let visualizer = Visualizer::new();
    visualizer.print(OwnedGraph);
}
