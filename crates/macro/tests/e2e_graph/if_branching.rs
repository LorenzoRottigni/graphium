use graphium;
use graphium_macro::{graph, node};

#[derive(Default, PartialEq, Eq, Debug, Copy, Clone)]
pub enum Status {
    #[default]
    Success,
    Fail,
    Retry,
}

#[test]
fn e2e_graph_if_elif_else_outputs() {
    let mut ctx = graphium::Context::default();

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
        IfGraph<graphium::Context> -> (result: u32) {
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

    let result = IfGraph::run(&mut ctx);
    assert_eq!(result, 10);
}

#[test]
fn e2e_graph_if_elif_else_borrow() {
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
        IfBorrowGraph<Context> {
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

    IfBorrowGraph::run(&mut ctx);
}

#[test]
fn e2e_graph_if_elif_else_move() {
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
        IfMoveGraph<graphium::Context> {
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

    IfMoveGraph::run(&mut ctx);
}

#[test]
fn e2e_graph_if_elif_else_nested() {
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
        IfNestedGraph<Context> {
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

    IfNestedGraph::run(&mut ctx);
}

#[test]
fn e2e_graph_if_multiple_params() {
    let mut ctx = graphium::Context::default();

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
        IfMultiParamGraph<graphium::Context> {
            GetInputs() -> (status, count) >>
            @if |status: Status, count: u32| status == Status::Success && count == 2 {
                OnSuccess(status, count)
            }
            @else {
                OnFail()
            }
        }
    }

    IfMultiParamGraph::run(&mut ctx);
}
