use std::sync::LazyLock;
use std::time::Instant;

use prometheus::{
    Encoder, Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, Opts, Registry,
    TextEncoder,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct MetricConfig {
    pub performance: bool,
    pub errors: bool,
    pub count: bool,
    pub caller: bool,
    pub success_rate: bool,
    pub fail_rate: bool,
}

pub struct NodeMetricsHandle {
    cfg: MetricConfig,
    count: Option<IntCounter>,
    errors: Option<IntCounter>,
    success: Option<IntCounter>,
    fail: Option<IntCounter>,
    latency: Option<Histogram>,
}

impl NodeMetricsHandle {
    pub fn start_timer(&self) -> Option<Instant> {
        if self.cfg.performance {
            Some(Instant::now())
        } else {
            None
        }
    }

    pub fn record_success(&self, start: Option<Instant>) {
        if let Some(counter) = &self.count {
            counter.inc();
        }
        if let Some(counter) = &self.success {
            counter.inc();
        }
        if let (Some(hist), Some(started)) = (&self.latency, start) {
            hist.observe(started.elapsed().as_secs_f64());
        }
    }

    pub fn record_failure(&self, start: Option<Instant>) {
        if let Some(counter) = &self.count {
            counter.inc();
        }
        if let Some(counter) = &self.errors {
            counter.inc();
        }
        if let Some(counter) = &self.fail {
            counter.inc();
        }
        if let (Some(hist), Some(started)) = (&self.latency, start) {
            hist.observe(started.elapsed().as_secs_f64());
        }
    }
}

pub struct GraphMetricsHandle {
    cfg: MetricConfig,
    count: Option<IntCounter>,
    errors: Option<IntCounter>,
    success: Option<IntCounter>,
    fail: Option<IntCounter>,
    latency: Option<Histogram>,
}

impl GraphMetricsHandle {
    pub fn start_timer(&self) -> Option<Instant> {
        if self.cfg.performance {
            Some(Instant::now())
        } else {
            None
        }
    }

    pub fn record_success(&self, start: Option<Instant>) {
        if let Some(counter) = &self.count {
            counter.inc();
        }
        if let Some(counter) = &self.success {
            counter.inc();
        }
        if let (Some(hist), Some(started)) = (&self.latency, start) {
            hist.observe(started.elapsed().as_secs_f64());
        }
    }

    pub fn record_failure(&self, start: Option<Instant>) {
        if let Some(counter) = &self.count {
            counter.inc();
        }
        if let Some(counter) = &self.errors {
            counter.inc();
        }
        if let Some(counter) = &self.fail {
            counter.inc();
        }
        if let (Some(hist), Some(started)) = (&self.latency, start) {
            hist.observe(started.elapsed().as_secs_f64());
        }
    }
}

pub fn node_metrics(
    graph: &'static str,
    node: &'static str,
    caller: &'static str,
    cfg: MetricConfig,
) -> NodeMetricsHandle {
    let caller_label = cfg.caller.then_some(caller);
    let count = cfg
        .count
        .then(|| METRICS.node_count(graph, node, caller_label));
    let errors = cfg
        .errors
        .then(|| METRICS.node_errors(graph, node, caller_label));
    let success = cfg
        .success_rate
        .then(|| METRICS.node_success(graph, node, caller_label));
    let fail = cfg
        .fail_rate
        .then(|| METRICS.node_fail(graph, node, caller_label));
    let latency = cfg
        .performance
        .then(|| METRICS.node_latency(graph, node, caller_label));

    NodeMetricsHandle {
        cfg,
        count,
        errors,
        success,
        fail,
        latency,
    }
}

pub fn graph_metrics(
    graph: &'static str,
    caller: &'static str,
    cfg: MetricConfig,
) -> GraphMetricsHandle {
    let caller_label = cfg.caller.then_some(caller);
    let count = cfg.count.then(|| METRICS.graph_count(graph, caller_label));
    let errors = cfg
        .errors
        .then(|| METRICS.graph_errors(graph, caller_label));
    let success = cfg
        .success_rate
        .then(|| METRICS.graph_success(graph, caller_label));
    let fail = cfg
        .fail_rate
        .then(|| METRICS.graph_fail(graph, caller_label));
    let latency = cfg
        .performance
        .then(|| METRICS.graph_latency(graph, caller_label));

    GraphMetricsHandle {
        cfg,
        count,
        errors,
        success,
        fail,
        latency,
    }
}

pub fn render_prometheus() -> String {
    let metric_families = METRICS.registry.gather();
    let mut buffer = Vec::new();
    TextEncoder::new()
        .encode(&metric_families, &mut buffer)
        .expect("prometheus encode failed");
    String::from_utf8(buffer).expect("prometheus text must be utf8")
}

struct MetricsRegistry {
    registry: Registry,

    node_count: IntCounterVec,
    node_count_caller: IntCounterVec,
    node_errors: IntCounterVec,
    node_errors_caller: IntCounterVec,
    node_success: IntCounterVec,
    node_success_caller: IntCounterVec,
    node_fail: IntCounterVec,
    node_fail_caller: IntCounterVec,
    node_latency: HistogramVec,
    node_latency_caller: HistogramVec,

    graph_count: IntCounterVec,
    graph_count_caller: IntCounterVec,
    graph_errors: IntCounterVec,
    graph_errors_caller: IntCounterVec,
    graph_success: IntCounterVec,
    graph_success_caller: IntCounterVec,
    graph_fail: IntCounterVec,
    graph_fail_caller: IntCounterVec,
    graph_latency: HistogramVec,
    graph_latency_caller: HistogramVec,
}

impl MetricsRegistry {
    fn new() -> Self {
        let registry = Registry::new();

        let node_count = IntCounterVec::new(
            Opts::new("graphium_node_count_total", "Total node executions"),
            &["graph", "node"],
        )
        .expect("valid node_count metric");
        let node_count_caller = IntCounterVec::new(
            Opts::new(
                "graphium_node_count_by_caller_total",
                "Total node executions by caller",
            ),
            &["graph", "node", "caller"],
        )
        .expect("valid node_count_caller metric");
        let node_errors = IntCounterVec::new(
            Opts::new("graphium_node_errors_total", "Total node failed executions"),
            &["graph", "node"],
        )
        .expect("valid node_errors metric");
        let node_errors_caller = IntCounterVec::new(
            Opts::new(
                "graphium_node_errors_by_caller_total",
                "Total node failed executions by caller",
            ),
            &["graph", "node", "caller"],
        )
        .expect("valid node_errors_caller metric");
        let node_success = IntCounterVec::new(
            Opts::new(
                "graphium_node_success_total",
                "Total node successful executions",
            ),
            &["graph", "node"],
        )
        .expect("valid node_success metric");
        let node_success_caller = IntCounterVec::new(
            Opts::new(
                "graphium_node_success_by_caller_total",
                "Total node successful executions by caller",
            ),
            &["graph", "node", "caller"],
        )
        .expect("valid node_success_caller metric");
        let node_fail = IntCounterVec::new(
            Opts::new("graphium_node_fail_total", "Total node failed executions"),
            &["graph", "node"],
        )
        .expect("valid node_fail metric");
        let node_fail_caller = IntCounterVec::new(
            Opts::new(
                "graphium_node_fail_by_caller_total",
                "Total node failed executions by caller",
            ),
            &["graph", "node", "caller"],
        )
        .expect("valid node_fail_caller metric");
        let node_latency = HistogramVec::new(
            HistogramOpts::new(
                "graphium_node_latency_seconds",
                "Node execution latency in seconds",
            ),
            &["graph", "node"],
        )
        .expect("valid node_latency metric");
        let node_latency_caller = HistogramVec::new(
            HistogramOpts::new(
                "graphium_node_latency_by_caller_seconds",
                "Node execution latency in seconds by caller",
            ),
            &["graph", "node", "caller"],
        )
        .expect("valid node_latency_caller metric");

        let graph_count = IntCounterVec::new(
            Opts::new("graphium_graph_count_total", "Total graph executions"),
            &["graph"],
        )
        .expect("valid graph_count metric");
        let graph_count_caller = IntCounterVec::new(
            Opts::new(
                "graphium_graph_count_by_caller_total",
                "Total graph executions by caller",
            ),
            &["graph", "caller"],
        )
        .expect("valid graph_count_caller metric");
        let graph_errors = IntCounterVec::new(
            Opts::new(
                "graphium_graph_errors_total",
                "Total graph failed executions",
            ),
            &["graph"],
        )
        .expect("valid graph_errors metric");
        let graph_errors_caller = IntCounterVec::new(
            Opts::new(
                "graphium_graph_errors_by_caller_total",
                "Total graph failed executions by caller",
            ),
            &["graph", "caller"],
        )
        .expect("valid graph_errors_caller metric");
        let graph_success = IntCounterVec::new(
            Opts::new(
                "graphium_graph_success_total",
                "Total graph successful executions",
            ),
            &["graph"],
        )
        .expect("valid graph_success metric");
        let graph_success_caller = IntCounterVec::new(
            Opts::new(
                "graphium_graph_success_by_caller_total",
                "Total graph successful executions by caller",
            ),
            &["graph", "caller"],
        )
        .expect("valid graph_success_caller metric");
        let graph_fail = IntCounterVec::new(
            Opts::new("graphium_graph_fail_total", "Total graph failed executions"),
            &["graph"],
        )
        .expect("valid graph_fail metric");
        let graph_fail_caller = IntCounterVec::new(
            Opts::new(
                "graphium_graph_fail_by_caller_total",
                "Total graph failed executions by caller",
            ),
            &["graph", "caller"],
        )
        .expect("valid graph_fail_caller metric");
        let graph_latency = HistogramVec::new(
            HistogramOpts::new(
                "graphium_graph_latency_seconds",
                "Graph execution latency in seconds",
            ),
            &["graph"],
        )
        .expect("valid graph_latency metric");
        let graph_latency_caller = HistogramVec::new(
            HistogramOpts::new(
                "graphium_graph_latency_by_caller_seconds",
                "Graph execution latency in seconds by caller",
            ),
            &["graph", "caller"],
        )
        .expect("valid graph_latency_caller metric");

        registry
            .register(Box::new(node_count.clone()))
            .expect("register node_count");
        registry
            .register(Box::new(node_count_caller.clone()))
            .expect("register node_count_caller");
        registry
            .register(Box::new(node_errors.clone()))
            .expect("register node_errors");
        registry
            .register(Box::new(node_errors_caller.clone()))
            .expect("register node_errors_caller");
        registry
            .register(Box::new(node_success.clone()))
            .expect("register node_success");
        registry
            .register(Box::new(node_success_caller.clone()))
            .expect("register node_success_caller");
        registry
            .register(Box::new(node_fail.clone()))
            .expect("register node_fail");
        registry
            .register(Box::new(node_fail_caller.clone()))
            .expect("register node_fail_caller");
        registry
            .register(Box::new(node_latency.clone()))
            .expect("register node_latency");
        registry
            .register(Box::new(node_latency_caller.clone()))
            .expect("register node_latency_caller");

        registry
            .register(Box::new(graph_count.clone()))
            .expect("register graph_count");
        registry
            .register(Box::new(graph_count_caller.clone()))
            .expect("register graph_count_caller");
        registry
            .register(Box::new(graph_errors.clone()))
            .expect("register graph_errors");
        registry
            .register(Box::new(graph_errors_caller.clone()))
            .expect("register graph_errors_caller");
        registry
            .register(Box::new(graph_success.clone()))
            .expect("register graph_success");
        registry
            .register(Box::new(graph_success_caller.clone()))
            .expect("register graph_success_caller");
        registry
            .register(Box::new(graph_fail.clone()))
            .expect("register graph_fail");
        registry
            .register(Box::new(graph_fail_caller.clone()))
            .expect("register graph_fail_caller");
        registry
            .register(Box::new(graph_latency.clone()))
            .expect("register graph_latency");
        registry
            .register(Box::new(graph_latency_caller.clone()))
            .expect("register graph_latency_caller");

        Self {
            registry,
            node_count,
            node_count_caller,
            node_errors,
            node_errors_caller,
            node_success,
            node_success_caller,
            node_fail,
            node_fail_caller,
            node_latency,
            node_latency_caller,
            graph_count,
            graph_count_caller,
            graph_errors,
            graph_errors_caller,
            graph_success,
            graph_success_caller,
            graph_fail,
            graph_fail_caller,
            graph_latency,
            graph_latency_caller,
        }
    }

    fn node_count(
        &self,
        graph: &'static str,
        node: &'static str,
        caller: Option<&'static str>,
    ) -> IntCounter {
        match caller {
            Some(value) => self
                .node_count_caller
                .with_label_values(&[graph, node, value]),
            None => self.node_count.with_label_values(&[graph, node]),
        }
    }

    fn node_errors(
        &self,
        graph: &'static str,
        node: &'static str,
        caller: Option<&'static str>,
    ) -> IntCounter {
        match caller {
            Some(value) => self
                .node_errors_caller
                .with_label_values(&[graph, node, value]),
            None => self.node_errors.with_label_values(&[graph, node]),
        }
    }

    fn node_success(
        &self,
        graph: &'static str,
        node: &'static str,
        caller: Option<&'static str>,
    ) -> IntCounter {
        match caller {
            Some(value) => self
                .node_success_caller
                .with_label_values(&[graph, node, value]),
            None => self.node_success.with_label_values(&[graph, node]),
        }
    }

    fn node_fail(
        &self,
        graph: &'static str,
        node: &'static str,
        caller: Option<&'static str>,
    ) -> IntCounter {
        match caller {
            Some(value) => self
                .node_fail_caller
                .with_label_values(&[graph, node, value]),
            None => self.node_fail.with_label_values(&[graph, node]),
        }
    }

    fn node_latency(
        &self,
        graph: &'static str,
        node: &'static str,
        caller: Option<&'static str>,
    ) -> Histogram {
        match caller {
            Some(value) => self
                .node_latency_caller
                .with_label_values(&[graph, node, value]),
            None => self.node_latency.with_label_values(&[graph, node]),
        }
    }

    fn graph_count(&self, graph: &'static str, caller: Option<&'static str>) -> IntCounter {
        match caller {
            Some(value) => self.graph_count_caller.with_label_values(&[graph, value]),
            None => self.graph_count.with_label_values(&[graph]),
        }
    }

    fn graph_errors(&self, graph: &'static str, caller: Option<&'static str>) -> IntCounter {
        match caller {
            Some(value) => self.graph_errors_caller.with_label_values(&[graph, value]),
            None => self.graph_errors.with_label_values(&[graph]),
        }
    }

    fn graph_success(&self, graph: &'static str, caller: Option<&'static str>) -> IntCounter {
        match caller {
            Some(value) => self.graph_success_caller.with_label_values(&[graph, value]),
            None => self.graph_success.with_label_values(&[graph]),
        }
    }

    fn graph_fail(&self, graph: &'static str, caller: Option<&'static str>) -> IntCounter {
        match caller {
            Some(value) => self.graph_fail_caller.with_label_values(&[graph, value]),
            None => self.graph_fail.with_label_values(&[graph]),
        }
    }

    fn graph_latency(&self, graph: &'static str, caller: Option<&'static str>) -> Histogram {
        match caller {
            Some(value) => self.graph_latency_caller.with_label_values(&[graph, value]),
            None => self.graph_latency.with_label_values(&[graph]),
        }
    }
}

static METRICS: LazyLock<MetricsRegistry> = LazyLock::new(MetricsRegistry::new);
