use graphium_macro::{graph, graph_test, node, node_test};
use graphium_ui::{GraphiumUiConfig, graphs};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Success,
    Retry,
    Fail,
}

#[derive(Default)]
struct Context {
    a_number: u32,
    attempts: u32,
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    fn get_number() -> u32 {
        42
    }
}

node! {
    #[metrics("performance", "count", "caller")]
    fn duplicate(value: u32) -> (u32, u32) {
        (value, value)
    }
}

node! {
    #[metrics("performance", "count")]
    fn left_branch(value: u32) -> u32 {
        value + 1
    }
}

node! {
    #[metrics("performance", "count")]
    fn right_branch(value: u32) -> u32 {
        value + 2
    }
}

node! {
    #[metrics("performance", "count")]
    fn combine(left: u32, right: u32) -> u32 {
        left + right
    }
}

node! {
    #[metrics("performance", "count")]
    fn pipe_number(value: u32) -> u32 {
        value
    }
}

node! {
    #[metrics("performance", "count")]
    fn store_number(ctx: &mut Context, a_number: u32) {
        ctx.a_number = a_number;
    }
}

node! {
    #[metrics("performance", "count")]
    fn take_ownership(a_number: &u32) -> u32 {
        *a_number
    }
}

node! {
    #[metrics("performance", "count")]
    fn decide_status(sum: u32) -> (Status, u32) {
        if sum % 2 == 0 {
            (Status::Success, sum)
        } else if sum % 3 == 0 {
            (Status::Retry, sum)
        } else {
            (Status::Fail, sum)
        }
    }
}

node! {
    #[metrics("performance", "count")]
    fn on_success(sum: u32) -> u32 {
        sum * 2
    }
}

node! {
    #[metrics("performance", "count")]
    fn on_retry(sum: u32) -> u32 {
        sum + 5
    }
}

node! {
    #[metrics("performance", "count")]
    fn on_fail(sum: u32) -> u32 {
        sum
    }
}

node! {
    #[metrics("performance", "count")]
    fn init_attempts(ctx: &mut Context) {
        ctx.attempts = 0;
    }
}

node! {
    #[metrics("performance", "count")]
    fn bump_attempts(ctx: &mut Context) {
        ctx.attempts += 1;
    }
}

node! {
    #[metrics("performance", "count")]
    fn read_attempts(ctx: &Context) -> u32 {
        ctx.attempts
    }
}

node! {
    #[metrics("performance", "count")]
    fn status_from_attempt(attempt: u32) -> (Status, u32) {
        if attempt >= 3 {
            (Status::Success, attempt)
        } else {
            (Status::Retry, attempt)
        }
    }
}

node! {
    #[metrics("performance", "count")]
    fn route_success(attempt: u32) -> u32 {
        attempt * 10
    }
}

node! {
    #[metrics("performance", "count")]
    fn route_retry() -> u32 {
        0
    }
}

node! {
    #[metrics("performance", "count")]
    fn route_fail() -> u32 {
        999
    }
}

graph! {
    #[metadata(
        context = Context,
        inputs = (left: u32, right: u32),
        outputs = (left: u32, right: u32)
    )]
    #[metrics("performance", "count", "success_rate")]
    InnerGraph {
        PipeNumber(left) -> (left) & PipeNumber(right) -> (right) >>
        LeftBranch(left) -> (left) & RightBranch(right) -> (right)
    }
}

graph! {
    #[metadata(
        context = Context,
        inputs = (left: u32, right: u32),
        outputs = (left: u32, right: u32)
    )]
    #[metrics("performance", "count", "success_rate")]
    DeepInnerGraph {
        InnerGraph::run(left, right) -> (left, right) >>
        PipeNumber(left) -> (left) & PipeNumber(right) -> (right)
    }
}

graph! {
    #[metadata(context = Context, outputs = (a_split: u32))]
    #[metrics("performance", "errors", "count", "caller", "success_rate", "fail_rate")]
    OwnedGraph {
        GetNumber() -> (a_number) >>
        Duplicate(a_number) -> (left, right) >>
        LeftBranch(left) -> (left) & RightBranch(right) -> (right) >>
        DeepInnerGraph::run(left, right) -> (left, right) >>
        Combine(left, right) -> (sum) >>
        DecideStatus(sum) -> (status, sum) >>
        @if |status: Status| status == Status::Success -> (out) {
            OnSuccess(sum) -> (out)
        }
        @elif |status: Status| status == Status::Retry {
            OnRetry(sum) -> (out)
        }
        @else {
            OnFail(sum) -> (out)
        } >>
        PipeNumber(out) -> (a_split)
    }
}

graph! {
    #[metadata(context = Context, outputs = (a_number: u32))]
    #[metrics("performance", "count", "success_rate")]
    BorrowedGraph {
        GetNumber() -> (a_number) >>
        StoreNumber(a_number) -> (&a_number) >>
        TakeOwnership(&a_number) -> (a_number) >>
        PipeNumber(a_number) -> (a_number)
    }
}

graph! {
    #[metadata(context = Context, outputs = (a_number: u32))]
    #[metrics("performance", "count", "success_rate")]
    ControlFlowGraph {
        InitAttempts() >>
        @while |ctx: &Context| ctx.attempts < 3 {
            BumpAttempts()
        } >>
        ReadAttempts() -> (attempt) >>
        StatusFromAttempt(attempt) -> (status, attempt) >>
        @match status -> (result) {
            Status::Success => RouteSuccess(attempt) -> (result),
            Status::Retry => RouteRetry() -> (result),
            Status::Fail => RouteFail() -> (result),
        } >>
        PipeNumber(result) -> (a_number)
    }
}

pub fn config() -> GraphiumUiConfig {
    GraphiumUiConfig {
        prometheus_url: std::env::var("GRAPHIUM_PROMETHEUS_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9090".to_string()),
        graphs: graphs![OwnedGraph, BorrowedGraph, ControlFlowGraph],
        ..Default::default()
    }
}

node_test! {
    #[test]
    #[for_node(GetNumber)]
    fn get_number_returns_42() {
        let value = GetNumber::__graphium_run(&());
        assert_eq!(value, 42);
    }
}

node_test! {
    #[test]
    #[for_node(Duplicate)]
    fn duplicate_clones_value() {
        let (left, right) = Duplicate::__graphium_run(&(), 7);
        assert_eq!((left, right), (7, 7));
    }
}

graph_test! {
    #[test]
    #[for_graph(OwnedGraph)]
    fn owned_graph_returns_non_zero_split() {
        let mut ctx = Context::default();
        let out = OwnedGraph::__graphium_run(&mut ctx);
        assert!(out > 0);
    }
}

graph_test! {
    #[test]
    #[for_graph(BorrowedGraph)]
    fn borrowed_graph_keeps_ownership_path() {
        let mut ctx = Context::default();
        let out = BorrowedGraph::__graphium_run(&mut ctx);
        assert_eq!(out, 42);
    }
}

graph_test! {
    #[test]
    #[for_graph(ControlFlowGraph)]
    fn control_flow_graph_converges_to_success_path() {
        let mut ctx = Context::default();
        let out = ControlFlowGraph::__graphium_run(&mut ctx);
        assert_eq!(out, 30);
    }
}
