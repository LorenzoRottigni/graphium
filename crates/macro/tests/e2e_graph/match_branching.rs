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
        #[metadata(context = graphium::Context)]
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
        #[metadata(context = graphium::Context, outputs = (match_result: u32))]
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
