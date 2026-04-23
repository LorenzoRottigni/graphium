use graphium_macro::{graph, node};

#[test]
fn e2e_graph_metrics_api_emits_prometheus_metrics() {
    let mut ctx = graphium::Context::default();

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
        #[metrics("performance", "count", "success_rate")]
        MetricsGraph<graphium::Context> -> (result: u32) {
            Seed() -> (left, right) >>
            Sum(left, right) -> (result)
        }
    }

    let result = MetricsGraph::run(&mut ctx);
    assert_eq!(result, 3);

    let exported = graphium::metrics::render_prometheus();
    assert!(exported.contains("graphium_graph_count_total"));
    assert!(exported.contains("graphium_graph_success_total"));
    assert!(exported.contains("graphium_graph_latency_seconds"));
    assert!(exported.contains("graphium_node_count_total"));
    assert!(exported.contains("graphium_node_success_by_caller_total"));
    assert!(exported.contains("graphium_node_latency_by_caller_seconds"));
}
