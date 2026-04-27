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

    // Ensure the telemetry singleton can be initialized and shut down.
    // The actual export happens asynchronously to the configured OTLP endpoints.
    #[cfg(any(feature = "metrics", feature = "logs", feature = "trace"))]
    graphium::GraphiumTelemetry::global().shutdown();
}
