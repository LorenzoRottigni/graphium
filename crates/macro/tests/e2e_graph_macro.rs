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

#[test]
// graph! macro should allow @if/@elif/@else branching with outputs.
fn e2e_graph_if_elif_else_outputs() {
    let mut ctx = Context::default();

    node! {
        fn get_operation_status() -> Status {
            Status::Success
        }
    }

    node! {
        fn on_success() -> u32 {
            10
        }
    }

    node! {
        fn on_fail() -> u32 {
            20
        }
    }

    node! {
        fn on_retry() -> u32 {
            30
        }
    }

    graph! {
        #[metadata(context = Context, outputs = (result: u32))]
        IfGraph {
            GetOperationStatus() -> (status) >>
            @if |status: Status| status == Status::Success -> (result) {
                OnSuccess() -> (result)
            }
            @elif |status: Status| status == Status::Fail {
                OnFail() -> (result)
            }
            @else {
                OnRetry() -> (result)
            }
        }
    }

    let result = IfGraph::__graphio_run(&mut ctx);
    assert_eq!(result, 10);
}

#[test]
// graph! macro should allow @if/@elif/@else branching with borrowed artifacts.
fn e2e_graph_if_elif_else_borrow() {
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
        IfBorrowGraph {
            GetOperationStatus() -> (&status) >>
            @if |ctx: &Context| ctx.status == Status::Success {
                OnSuccess(&status)
            }
            @elif |ctx: &Context| ctx.status == Status::Fail {
                OnFail()
            }
            @else {
                OnRetry()
            }
        }
    }

    IfBorrowGraph::__graphio_run(&mut ctx);
}

#[test]
// graph! macro should allow @if/@elif/@else branching with moved artifacts.
fn e2e_graph_if_elif_else_move() {
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
        IfMoveGraph {
            GetOperationStatus() -> (status) >>
            @if |status: Status| status == Status::Success {
                OnSuccess(status)
            }
            @elif |status: Status| status == Status::Fail {
                OnFail()
            }
            @else {
                OnRetry()
            }
        }
    }

    IfMoveGraph::__graphio_run(&mut ctx);
}

#[test]
// graph! macro should allow nested @if/@elif/@else branching.
fn e2e_graph_if_elif_else_nested() {
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
        IfNestedGraph {
            GetOperationStatus() -> (status) >>
            @if |status: Status| status == Status::Success {
                @if |ctx: &Context| ctx.status == Status::Success {
                    OnSuccess(status)
                }
                @elif |ctx: &Context| ctx.status == Status::Fail {
                    OnFail()
                }
                @else {
                    OnRetry()
                }
            }
            @elif |status: Status| status == Status::Fail {
                OnFail()
            }
            @else {
                OnRetry()
            }
        }
    }

    IfNestedGraph::__graphio_run(&mut ctx);
}

#[test]
// graph! macro should allow @if with multiple selector params.
fn e2e_graph_if_multiple_params() {
    let mut ctx = Context::default();

    node! {
        fn get_inputs() -> (Status, u32) {
            (Status::Success, 2)
        }
    }

    node! {
        fn on_success(status: Status, count: u32) {
            assert_eq!(status, Status::Success);
            assert_eq!(count, 2);
        }
    }

    node! {
        fn on_fail() {
            panic!("Graph branching failed")
        }
    }

    graph! {
        #[metadata(context = Context)]
        IfMultiParamGraph {
            GetInputs() -> (status, count) >>
            @if |status: Status, count: u32| status == Status::Success && count == 2 {
                OnSuccess(status, count)
            }
            @else {
                OnFail()
            }
        }
    }

    IfMultiParamGraph::__graphio_run(&mut ctx);
}
