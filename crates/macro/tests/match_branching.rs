use graphium_macro::{graph, node};

#[derive(Default, PartialEq, Eq, Debug, Copy, Clone)]
pub enum Status {
    #[default]
    Success,
    Fail,
    Retry,
}

#[test]
fn e2e_graph_branching_borrow() {
    #[derive(Default)]
    pub struct Context {
        pub status: Status,
    }

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
        ConditionalGraph<'a, Context> {
            GetOperationStatus() -> (&'a status) >>
            @match ctx.status {
                Status::Success => OnSuccess(&'a status),
                Status::Fail => OnFail(),
                Status::Retry => OnRetry(),
            }
        }
    }

    ConditionalGraph::run(&mut ctx);
}

#[test]
fn e2e_graph_branching_move() {
    let mut ctx = graphium::Context::default();

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
        ConditionalGraph<'a, graphium::Context> {
            GetOperationStatus() -> (status) >>
            @match status {
                Status::Success => OnSuccess(status),
                Status::Fail => OnFail(),
                Status::Retry => OnRetry(),
            }
        }
    }

    ConditionalGraph::run(&mut ctx);
}

#[test]
fn e2e_graph_nested_branching() {
    #[derive(Default)]
    pub struct Context {
        pub status: Status,
    }

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
        NestedConditionalGraph<'a, Context> {
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

    NestedConditionalGraph::run(&mut ctx);
}

#[test]
fn e2e_graph_match_outputs() {
    let mut ctx = graphium::Context::default();

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
        MatchOutputGraph<'a, graphium::Context> -> (match_result: u32) {
            GetOperationStatus() -> (status) >>
            @match status -> (match_result) {
                Status::Success => OnSuccess() -> (match_result),
                Status::Fail => OnFail() -> (match_result),
                Status::Retry => OnRetry() -> (match_result),
            }
        }
    }

    let result = MatchOutputGraph::run(&mut ctx);
    assert_eq!(result, 42);
}
