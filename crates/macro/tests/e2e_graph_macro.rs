pub mod data;

use data::ctx::{Context, Status};
use futures::executor::block_on;
use graphium_macro::{graph, node};
use std::time::{Duration, Instant};

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
    let duplicated = OwnedGraph::__graphium_run(&mut ctx);

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
    let num = BorrowedGraph::__graphium_run(&mut ctx);
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

    CtxGraph::__graphium_run(&mut ctx);
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

    ConditionalGraph::__graphium_run(&mut ctx);
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

    ConditionalGraph::__graphium_run(&mut ctx);
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

    NestedConditionalGraph::__graphium_run(&mut ctx);
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

    let result = MatchOutputGraph::__graphium_run(&mut ctx);
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

    let result = IfGraph::__graphium_run(&mut ctx);
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

    IfBorrowGraph::__graphium_run(&mut ctx);
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

    IfMoveGraph::__graphium_run(&mut ctx);
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

    IfNestedGraph::__graphium_run(&mut ctx);
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

    IfMultiParamGraph::__graphium_run(&mut ctx);
}

#[test]
// graph! macro should allow @while loops.
fn e2e_graph_while_loop() {
    let mut ctx = Context::default();

    node! {
        fn init_ctx(ctx: &mut Context) {
            ctx.a_number = 0;
        }
    }

    node! {
        fn inc_ctx(ctx: &mut Context) {
            ctx.a_number += 1;
        }
    }

    graph! {
        #[metadata(context = Context)]
        WhileGraph {
            InitCtx() >>
            @while |ctx: &Context| ctx.a_number < 3 {
                IncCtx()
            }
        }
    }

    WhileGraph::__graphium_run(&mut ctx);
    assert_eq!(ctx.a_number, 3);
}

#[test]
// graph! macro should allow @loop with @break.
fn e2e_graph_loop_with_break() {
    let mut ctx = Context::default();

    node! {
        fn init_ctx(ctx: &mut Context) {
            ctx.a_number = 0;
        }
    }

    node! {
        fn inc_ctx(ctx: &mut Context) {
            ctx.a_number += 1;
        }
    }

    node! {
        fn noop() {}
    }

    graph! {
        #[metadata(context = Context)]
        LoopBreakGraph {
            InitCtx() >>
            @loop {
                IncCtx() >>
                @if |ctx: &Context| ctx.a_number >= 3 {
                    @break
                }
                @else {
                    Noop()
                }
            }
        }
    }

    LoopBreakGraph::__graphium_run(&mut ctx);
    assert_eq!(ctx.a_number, 3);
}

#[test]
// graph! macro should allow async graphs with sync nodes.
fn e2e_graph_async_with_sync_nodes() {
    let mut ctx = Context::default();

    node! {
        fn set_ctx(ctx: &mut Context) {
            ctx.a_number = 5;
        }
    }

    graph! {
        #[metadata(context = Context, async = true)]
        AsyncSyncGraph {
            SetCtx()
        }
    }

    block_on(AsyncSyncGraph::__graphium_run_async(&mut ctx));
    assert_eq!(ctx.a_number, 5);
}

#[test]
// graph! macro should allow async nodes inside async graphs.
fn e2e_graph_async_nodes() {
    let mut ctx = Context::default();

    node! {
        async fn get_number() -> u32 {
            7
        }
    }

    node! {
        async fn add_one(value: u32) -> u32 {
            value + 1
        }
    }

    graph! {
        #[metadata(context = Context, outputs = (a_number: u32), async = true)]
        AsyncGraph {
            GetNumber() -> (a_number) >>
            AddOne(a_number) -> (a_number)
        }
    }

    let value = block_on(AsyncGraph::__graphium_run_async(&mut ctx));
    assert_eq!(value, 8);
}

#[test]
// graph! `&` should run sibling branches in real parallelism for sync graphs.
fn e2e_graph_parallel_branches_run_concurrently() {
    let mut ctx = Context::default();

    node! {
        fn left_work() -> u32 {
            std::thread::sleep(Duration::from_millis(200));
            1
        }
    }

    node! {
        fn right_work() -> u32 {
            std::thread::sleep(Duration::from_millis(200));
            2
        }
    }

    node! {
        #[metrics(
            "performance",
            "errors",
            "count",
            "caller",
            "success_rate",
            "fail_rate"
        )]
        fn sum(left: u32, right: u32) -> u32 {
            left + right
        }
    }

    graph! {
        #[metadata(context = Context, outputs = (value: u32))]
        ParallelGraph {
            LeftWork() -> (left) & RightWork() -> (right) >>
            Sum(left, right) -> (value)
        }
    }

    let start = Instant::now();
    let value = ParallelGraph::__graphium_run(&mut ctx);
    let elapsed = start.elapsed();

    assert_eq!(value, 3);
    assert!(
        elapsed < Duration::from_millis(350),
        "parallel branches took too long: {:?}",
        elapsed
    );
}

#[test]
// graph! and node! metrics attributes should emit prometheus counters/histograms.
fn e2e_graph_metrics_api_emits_prometheus_metrics() {
    let mut ctx = Context::default();

    node! {
        #[metrics("performance", "errors", "count", "caller", "success_rate", "fail_rate")]
        fn sum(left: u32, right: u32) -> u32 {
            left + right
        }
    }

    node! {
        #[metrics("count")]
        fn seed() -> (u32, u32) {
            (1, 2)
        }
    }

    graph! {
        #[metadata(context = Context, outputs = (result: u32))]
        #[metrics("performance", "count", "success_rate")]
        MetricsGraph {
            Seed() -> (left, right) >>
            Sum(left, right) -> (result)
        }
    }

    let result = MetricsGraph::__graphium_run(&mut ctx);
    assert_eq!(result, 3);

    let exported = graphium::metrics::render_prometheus();
    assert!(exported.contains("graphium_graph_count_total"));
    assert!(exported.contains("graphium_graph_success_total"));
    assert!(exported.contains("graphium_graph_latency_seconds"));
    assert!(exported.contains("graphium_node_count_total"));
    assert!(exported.contains("graphium_node_success_by_caller_total"));
    assert!(exported.contains("graphium_node_latency_by_caller_seconds"));
}
