pub mod data;

use data::ctx::{Context, Status};
use graphio_macro::{graph, node};

/// graph! macro should propagate owned artifacts across nodes
#[test]
fn e2e_graph_macro_moves_artifacts() {
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
fn e2e_graph_macro_borrows_artifacts() {
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
fn e2e_graph_macro_wires_ctx() {
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

#[test]
// graph! macro should allow conditional branching using @match modifier based on ctx.
fn e2e_graph_branching_borrow() {
    let mut ctx = Context::default();

    node! {
        fn get_operation_status() -> Status {
            Status::Success
        }
    }

    node! {
        fn on_success(status: &Status) {
            assert_eq!(*status, Status::Success)
        }
    }

    node! {
        fn on_fail() {
            panic!("Graph branching failed")
        }
    }

    node! {
        fn on_retry() {
            panic!("Graph branching failed")
        }
    }

    graph! {
        #[metadata(context = Context)]
        ConditionalGraph {
            GetOperationStatus() -> (&status) >>
            @match ctx.status {
                Status::Success => OnSuccess(&status),
                Status::Fail => OnFail(),
                Status::Retry => OnRetry(),
            }
        }
    }

    ConditionalGraph::__graphio_run(&mut ctx);
}

#[test]
// graph! macro should allow conditional branching using @match modifier based on ctx.
fn e2e_graph_branching_move() {
    let mut ctx = Context::default();

    node! {
        fn get_operation_status() -> Status {
            Status::Success
        }
    }

    node! {
        fn on_success(status: Status) {
            assert_eq!(status, Status::Success)
        }
    }

    node! {
        fn on_fail() {
            panic!("Graph branching failed")
        }
    }

    node! {
        fn on_retry() {
            panic!("Graph branching failed")
        }
    }

    graph! {
        #[metadata(context = Context)]
        ConditionalGraph {
            GetOperationStatus() -> (status) >>
            @match status {
                Status::Success => OnSuccess(status),
                Status::Fail => OnFail(),
                Status::Retry => OnRetry(),
            }
        }
    }

    ConditionalGraph::__graphio_run(&mut ctx);
}

#[test]
// graph! macro should allow nested @match branching.
fn e2e_graph_nested_branching() {
    let mut ctx = Context::default();

    node! {
        fn get_operation_status() -> Status {
            Status::Success
        }
    }

    node! {
        fn on_success(status: Status) {
            assert_eq!(status, Status::Success)
        }
    }

    node! {
        fn on_fail() {
            panic!("Graph branching failed")
        }
    }

    node! {
        fn on_retry() {
            panic!("Graph branching failed")
        }
    }

    graph! {
        #[metadata(context = Context)]
        NestedConditionalGraph {
            GetOperationStatus() -> (status) >>
            @match status {
                Status::Success => @match ctx.status {
                    Status::Success => OnSuccess(status),
                    Status::Fail => OnFail(),
                    Status::Retry => OnRetry(),
                },
                Status::Fail => OnFail(),
                Status::Retry => OnRetry(),
            }
        }
    }

    NestedConditionalGraph::__graphio_run(&mut ctx);
}

#[test]
// graph! macro should allow @match to declare explicit outputs.
fn e2e_graph_match_outputs() {
    let mut ctx = Context::default();

    node! {
        fn get_operation_status() -> Status {
            Status::Success
        }
    }

    node! {
        fn on_success() -> u32 {
            42
        }
    }

    node! {
        fn on_fail() -> u32 {
            0
        }
    }

    node! {
        fn on_retry() -> u32 {
            1
        }
    }

    graph! {
        #[metadata(context = Context, outputs = (match_result: u32))]
        MatchOutputGraph {
            GetOperationStatus() -> (status) >>
            @match status -> (match_result) {
                Status::Success => OnSuccess() -> (match_result),
                Status::Fail => OnFail() -> (match_result),
                Status::Retry => OnRetry() -> (match_result),
            }
        }
    }

    let result = MatchOutputGraph::__graphio_run(&mut ctx);
    assert_eq!(result, 42);
}
