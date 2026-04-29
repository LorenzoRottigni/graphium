use graphium_macro::{graph, node};
use std::thread;

#[test]
fn e2e_parallel_with_borrowed_artifact_runs_sequentially() {
    node! {
        fn get_number() -> u32 {
            1
        }
    }

    node! {
        fn left(_number: &u32) -> thread::ThreadId {
            thread::current().id()
        }
    }

    node! {
        fn right(_number: &u32) -> thread::ThreadId {
            thread::current().id()
        }
    }

    graph! {
        BorrowedParallelGraph<'a, graphium::Context> -> (left: thread::ThreadId, right: thread::ThreadId) {
            GetNumber() -> (&'a number) >>
            Left(&'a number) -> (left) && Right(&'a number) -> (right)
        }
    }

    let mut ctx = graphium::Context::default();
    let (left, right) = BorrowedParallelGraph::run(&mut ctx);
    let main = thread::current().id();
    assert_eq!(left, main);
    assert_eq!(right, main);
}

#[test]
fn e2e_parallel_with_owned_artifacts_threads() {
    // Mirror the semantics of `parallel.rs`, but validate that the two branches
    // actually execute off the main thread.
    node! {
        fn left_work() -> thread::ThreadId {
            thread::current().id()
        }
    }

    node! {
        fn right_work() -> thread::ThreadId {
            thread::current().id()
        }
    }

    graph! {
        OwnedParallelGraph<'a, graphium::Context> -> (left: thread::ThreadId, right: thread::ThreadId) {
            LeftWork() -> (left) && RightWork() -> (right)
        }
    }

    let mut ctx = graphium::Context::default();
    let (left, right) = OwnedParallelGraph::run(&mut ctx);
    let main = thread::current().id();
    assert_ne!(left, main);
    assert_ne!(right, main);
}

