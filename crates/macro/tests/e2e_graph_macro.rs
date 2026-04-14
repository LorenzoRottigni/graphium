pub mod data;

use data::ctx::Context;
use graphio_macro::{graph, node};

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
            data::node::StoreNumber(a_number) -> (&a_number) >>
            data::node::TakeOwnership(&a_number) -> (a_number) >>
            data::node::PipeNumber(a_number) -> (a_number)
        }
    }
    let num = BorrowedGraph::__graphio_run(&mut ctx);
    assert_eq!(num, 42);
}

#[test]
// graph! macro should provide ctx mutable or immutable reference wherever is required.
fn e2e_graph_macro_wires_nodes_ctx() {
    let mut ctx = Context::default();

    node! {
        fn get_mutable_ctx(ctx: &mut Context) {
            ctx.a_number = 42;
        }
    }

    node! {
        fn extract_from_ctx(ctx: &Context) -> u32 {
            ctx.a_number
        }
    }

    node! {
        fn assert_ctx(a_number: u32, ctx: &Context) {
            assert_eq!(a_number, ctx.a_number)
        }
    }

    node! {
        fn assert_ctx_2(ctx: &Context, a_number: u32) {
            assert_eq!(a_number, ctx.a_number)
        }
    }

    graph! {
        #[metadata(context = Context)]
        CtxGraph {
            GetMutableCtx() >>
            ExtractFromCtx() -> (a_number) >>
            AssertCtx(a_number) & AssertCtx2(a_number)
        }
    }

    CtxGraph::__graphio_run(&mut ctx);
}
