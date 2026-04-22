use graphium_macro::{graph, node};
use std::time::{Duration, Instant};

#[test]
fn e2e_graph_parallel_branches_run_concurrently() {
    let mut ctx = graphium::Context::default();

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
        #[metadata(context = graphium::Context)]
        ParallelGraph -> (value: u32) {
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
