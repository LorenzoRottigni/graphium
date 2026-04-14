pub mod data;

use data::ctx::Context;
use graphio_macro::graph;

/// graph! macro should propagate owned artifacts across nodes
#[test]
fn e2e_graph_macro_owned_artifacts() {
    let mut ctx = Context::default();

    graph! {
        #[metadata(context = Context, outputs = (a_split: u32))]
        OwnedGraph {
            data::node::GetNumber() -> (a_number) >>
            data::node::Duplicate(a_number) -> (a_split, b_split) >>
            data::node::PipeNumber(a_split) -> (a_split)
        }
    }
    let duplicated = OwnedGraph::__graphio_run(&mut ctx);

    assert_eq!(duplicated, 42);
}

#[test]
/// graph! macro should propagate borrowed artifacts across nodes using context
fn e2e_graph_macro_borrowed_artifacts() {
    let mut ctx = Context::default();

    graph! {
        #[metadata(context = Context, outputs = (a_number: u32))]
        BorrowedGraph {
            data::node::GetNumber() -> (a_number) >>
            data::node::StoreNumber(a_number) >>
            data::node::TakeOwnership() -> (a_number) >>
            data::node::PipeNumber(a_number) -> (a_number)
        }
    }
    let num = BorrowedGraph::__graphio_run(&mut ctx);
    assert_eq!(num, 42);
}
