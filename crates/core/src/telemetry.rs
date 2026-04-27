use std::sync::OnceLock;
use std::time::Instant;

use opentelemetry::KeyValue;
use opentelemetry_sdk::Resource;

/// Re-export so macro-generated code can use `tracing::*` without requiring
/// a direct dependency in the consumer crate.
pub use tracing;

/// Default local endpoints for exporting telemetry.
///
/// These are intended for local docker-compose / k8s port-forward setups.
/// They can be overridden with [`TelemetryEndpoints`].
pub const DEFAULT_PROMETHEUS_OTLP_HTTP: &str = "http://127.0.0.1:9090/api/v1/otlp/v1/metrics";
pub const DEFAULT_LOKI_OTLP_HTTP: &str = "http://127.0.0.1:3100/otlp/v1/logs";
pub const DEFAULT_TEMPO_OTLP_HTTP: &str = "http://127.0.0.1:4318/v1/traces";

#[derive(Clone, Debug)]
pub struct TelemetryEndpoints {
    pub prometheus_otlp_http: String,
    pub loki_otlp_http: String,
    pub tempo_otlp_http: String,
    pub service_name: String,
}

impl TelemetryEndpoints {
    pub fn from_env() -> Self {
        fn env(key: &str) -> Option<String> {
            std::env::var(key).ok().filter(|v| !v.trim().is_empty())
        }

        fn join_base(base: &str, suffix: &str) -> String {
            let base = base.trim_end_matches('/');
            format!("{base}{suffix}")
        }

        let base_otlp = env("OTEL_EXPORTER_OTLP_ENDPOINT");

        let prometheus_otlp_http = env("GRAPHIUM_PROMETHEUS_OTLP_HTTP")
            .or_else(|| env("OTEL_EXPORTER_OTLP_METRICS_ENDPOINT"))
            .or_else(|| base_otlp.as_deref().map(|b| join_base(b, "/v1/metrics")))
            .unwrap_or_else(|| DEFAULT_PROMETHEUS_OTLP_HTTP.to_string());

        let loki_otlp_http = env("GRAPHIUM_LOKI_OTLP_HTTP")
            .or_else(|| env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT"))
            .or_else(|| base_otlp.as_deref().map(|b| join_base(b, "/v1/logs")))
            .unwrap_or_else(|| DEFAULT_LOKI_OTLP_HTTP.to_string());

        let tempo_otlp_http = env("GRAPHIUM_TEMPO_OTLP_HTTP")
            .or_else(|| env("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT"))
            .or_else(|| base_otlp.as_deref().map(|b| join_base(b, "/v1/traces")))
            .unwrap_or_else(|| DEFAULT_TEMPO_OTLP_HTTP.to_string());

        let service_name = env("GRAPHIUM_SERVICE_NAME")
            .or_else(|| env("OTEL_SERVICE_NAME"))
            .unwrap_or_else(|| "graphium".to_string());

        Self {
            prometheus_otlp_http,
            loki_otlp_http,
            tempo_otlp_http,
            service_name,
        }
    }
}

impl Default for TelemetryEndpoints {
    fn default() -> Self {
        Self::from_env()
    }
}

/// Mirror of the legacy `#[metrics(...)]` API.
///
/// This config is attached to macro-generated graph/node wrappers and controls
/// which metrics are emitted.
#[derive(Clone, Copy, Debug, Default)]
pub struct MetricConfig {
    pub performance: bool,
    pub errors: bool,
    pub count: bool,
    pub caller: bool,
    pub success_rate: bool,
    pub fail_rate: bool,
}

pub struct NodeTelemetryHandle {
    cfg: MetricConfig,

    #[cfg(feature = "metrics")]
    count: Option<opentelemetry::metrics::Counter<u64>>,
    #[cfg(feature = "metrics")]
    errors: Option<opentelemetry::metrics::Counter<u64>>,
    #[cfg(feature = "metrics")]
    success: Option<opentelemetry::metrics::Counter<u64>>,
    #[cfg(feature = "metrics")]
    fail: Option<opentelemetry::metrics::Counter<u64>>,
    #[cfg(feature = "metrics")]
    latency: Option<opentelemetry::metrics::Histogram<f64>>,

    #[cfg(feature = "metrics")]
    attrs: Vec<KeyValue>,
}

impl NodeTelemetryHandle {
    pub fn start_timer(&self) -> Option<Instant> {
        if self.cfg.performance {
            Some(Instant::now())
        } else {
            None
        }
    }

    pub fn record_success(&self, start: Option<Instant>) {
        #[cfg(not(feature = "metrics"))]
        let _ = start;
        #[cfg(feature = "metrics")]
        {
            if let Some(counter) = &self.count {
                counter.add(1, &self.attrs);
            }
            if let Some(counter) = &self.success {
                counter.add(1, &self.attrs);
            }
            if let (Some(hist), Some(started)) = (&self.latency, start) {
                hist.record(started.elapsed().as_secs_f64(), &self.attrs);
            }
        }
    }

    pub fn record_failure(&self, start: Option<Instant>) {
        #[cfg(not(feature = "metrics"))]
        let _ = start;
        #[cfg(feature = "metrics")]
        {
            if let Some(counter) = &self.count {
                counter.add(1, &self.attrs);
            }
            if let Some(counter) = &self.errors {
                counter.add(1, &self.attrs);
            }
            if let Some(counter) = &self.fail {
                counter.add(1, &self.attrs);
            }
            if let (Some(hist), Some(started)) = (&self.latency, start) {
                hist.record(started.elapsed().as_secs_f64(), &self.attrs);
            }
        }
    }
}

pub struct GraphTelemetryHandle {
    cfg: MetricConfig,

    #[cfg(feature = "metrics")]
    count: Option<opentelemetry::metrics::Counter<u64>>,
    #[cfg(feature = "metrics")]
    errors: Option<opentelemetry::metrics::Counter<u64>>,
    #[cfg(feature = "metrics")]
    success: Option<opentelemetry::metrics::Counter<u64>>,
    #[cfg(feature = "metrics")]
    fail: Option<opentelemetry::metrics::Counter<u64>>,
    #[cfg(feature = "metrics")]
    latency: Option<opentelemetry::metrics::Histogram<f64>>,

    #[cfg(feature = "metrics")]
    attrs: Vec<KeyValue>,
}

impl GraphTelemetryHandle {
    pub fn start_timer(&self) -> Option<Instant> {
        if self.cfg.performance {
            Some(Instant::now())
        } else {
            None
        }
    }

    pub fn record_success(&self, start: Option<Instant>) {
        #[cfg(not(feature = "metrics"))]
        let _ = start;
        #[cfg(feature = "metrics")]
        {
            if let Some(counter) = &self.count {
                counter.add(1, &self.attrs);
            }
            if let Some(counter) = &self.success {
                counter.add(1, &self.attrs);
            }
            if let (Some(hist), Some(started)) = (&self.latency, start) {
                hist.record(started.elapsed().as_secs_f64(), &self.attrs);
            }
        }
    }

    pub fn record_failure(&self, start: Option<Instant>) {
        #[cfg(not(feature = "metrics"))]
        let _ = start;
        #[cfg(feature = "metrics")]
        {
            if let Some(counter) = &self.count {
                counter.add(1, &self.attrs);
            }
            if let Some(counter) = &self.errors {
                counter.add(1, &self.attrs);
            }
            if let Some(counter) = &self.fail {
                counter.add(1, &self.attrs);
            }
            if let (Some(hist), Some(started)) = (&self.latency, start) {
                hist.record(started.elapsed().as_secs_f64(), &self.attrs);
            }
        }
    }
}

#[cfg(feature = "metrics")]
#[derive(Clone)]
struct MetricInstruments {
    node_count: opentelemetry::metrics::Counter<u64>,
    node_count_by_caller: opentelemetry::metrics::Counter<u64>,
    node_errors: opentelemetry::metrics::Counter<u64>,
    node_errors_by_caller: opentelemetry::metrics::Counter<u64>,
    node_success: opentelemetry::metrics::Counter<u64>,
    node_success_by_caller: opentelemetry::metrics::Counter<u64>,
    node_fail: opentelemetry::metrics::Counter<u64>,
    node_fail_by_caller: opentelemetry::metrics::Counter<u64>,
    node_latency: opentelemetry::metrics::Histogram<f64>,
    node_latency_by_caller: opentelemetry::metrics::Histogram<f64>,

    graph_count: opentelemetry::metrics::Counter<u64>,
    graph_count_by_caller: opentelemetry::metrics::Counter<u64>,
    graph_errors: opentelemetry::metrics::Counter<u64>,
    graph_errors_by_caller: opentelemetry::metrics::Counter<u64>,
    graph_success: opentelemetry::metrics::Counter<u64>,
    graph_success_by_caller: opentelemetry::metrics::Counter<u64>,
    graph_fail: opentelemetry::metrics::Counter<u64>,
    graph_fail_by_caller: opentelemetry::metrics::Counter<u64>,
    graph_latency: opentelemetry::metrics::Histogram<f64>,
    graph_latency_by_caller: opentelemetry::metrics::Histogram<f64>,
}

/// OpenTelemetry wiring + helpers for macro-generated graphs.
///
/// `graph!` / `node!` expansion calls [`GraphiumTelemetry::global`] to ensure
/// providers are initialized exactly once, then lazily builds per-graph/per-node
/// handles for metrics emission.
#[derive(Clone)]
pub struct GraphiumTelemetry {
    endpoints: TelemetryEndpoints,
    resource: Resource,

    #[cfg(feature = "metrics")]
    meter_provider: opentelemetry_sdk::metrics::SdkMeterProvider,
    #[cfg(feature = "metrics")]
    instruments: MetricInstruments,

    #[cfg(feature = "trace")]
    tracer_provider: opentelemetry_sdk::trace::SdkTracerProvider,
    #[cfg(feature = "logs")]
    logger_provider: opentelemetry_sdk::logs::SdkLoggerProvider,
}

impl GraphiumTelemetry {
    pub fn global() -> &'static Self {
        Self::init_global(TelemetryEndpoints::default())
    }

    pub fn init_global(endpoints: TelemetryEndpoints) -> &'static Self {
        static TELEMETRY: OnceLock<GraphiumTelemetry> = OnceLock::new();
        TELEMETRY.get_or_init(|| GraphiumTelemetry::init(endpoints))
    }

    fn init(endpoints: TelemetryEndpoints) -> Self {
        let resource = Resource::builder()
            .with_service_name(endpoints.service_name.clone())
            .with_attributes([KeyValue::new("library.name", "graphium")])
            .build();

        #[cfg(feature = "trace")]
        let tracer_provider = init_traces(&resource, &endpoints);

        #[cfg(feature = "logs")]
        let logger_provider = init_logs(&resource, &endpoints);

        #[cfg(feature = "metrics")]
        let (meter_provider, instruments) = init_metrics(&resource, &endpoints);

        install_tracing_subscriber(
            #[cfg(feature = "trace")]
            &tracer_provider,
            #[cfg(feature = "logs")]
            &logger_provider,
        );

        Self {
            endpoints,
            resource,
            #[cfg(feature = "metrics")]
            meter_provider,
            #[cfg(feature = "metrics")]
            instruments,
            #[cfg(feature = "trace")]
            tracer_provider,
            #[cfg(feature = "logs")]
            logger_provider,
        }
    }

    pub fn graph_metrics(
        &'static self,
        graph: &'static str,
        caller: &'static str,
        cfg: MetricConfig,
    ) -> GraphTelemetryHandle {
        #[cfg(not(feature = "metrics"))]
        let _ = (graph, caller);
        #[cfg(feature = "metrics")]
        let attrs = {
            let mut attrs = vec![KeyValue::new("graph", graph)];
            if cfg.caller {
                attrs.push(KeyValue::new("caller", caller));
            }
            attrs
        };

        #[cfg(feature = "metrics")]
        let (count, errors, success, fail, latency) = if cfg.caller {
            (
                cfg.count.then_some(self.instruments.graph_count_by_caller.clone()),
                cfg.errors
                    .then_some(self.instruments.graph_errors_by_caller.clone()),
                cfg.success_rate
                    .then_some(self.instruments.graph_success_by_caller.clone()),
                cfg.fail_rate
                    .then_some(self.instruments.graph_fail_by_caller.clone()),
                cfg.performance
                    .then_some(self.instruments.graph_latency_by_caller.clone()),
            )
        } else {
            (
                cfg.count.then_some(self.instruments.graph_count.clone()),
                cfg.errors.then_some(self.instruments.graph_errors.clone()),
                cfg.success_rate
                    .then_some(self.instruments.graph_success.clone()),
                cfg.fail_rate.then_some(self.instruments.graph_fail.clone()),
                cfg.performance.then_some(self.instruments.graph_latency.clone()),
            )
        };

        GraphTelemetryHandle {
            cfg,
            #[cfg(feature = "metrics")]
            count,
            #[cfg(feature = "metrics")]
            errors,
            #[cfg(feature = "metrics")]
            success,
            #[cfg(feature = "metrics")]
            fail,
            #[cfg(feature = "metrics")]
            latency,
            #[cfg(feature = "metrics")]
            attrs,
        }
    }

    pub fn node_metrics(
        &'static self,
        graph: &'static str,
        node: &'static str,
        caller: &'static str,
        cfg: MetricConfig,
    ) -> NodeTelemetryHandle {
        #[cfg(not(feature = "metrics"))]
        let _ = (graph, node, caller);
        #[cfg(feature = "metrics")]
        let attrs = {
            let mut attrs = vec![KeyValue::new("graph", graph), KeyValue::new("node", node)];
            if cfg.caller {
                attrs.push(KeyValue::new("caller", caller));
            }
            attrs
        };

        #[cfg(feature = "metrics")]
        let (count, errors, success, fail, latency) = if cfg.caller {
            (
                cfg.count.then_some(self.instruments.node_count_by_caller.clone()),
                cfg.errors
                    .then_some(self.instruments.node_errors_by_caller.clone()),
                cfg.success_rate
                    .then_some(self.instruments.node_success_by_caller.clone()),
                cfg.fail_rate
                    .then_some(self.instruments.node_fail_by_caller.clone()),
                cfg.performance
                    .then_some(self.instruments.node_latency_by_caller.clone()),
            )
        } else {
            (
                cfg.count.then_some(self.instruments.node_count.clone()),
                cfg.errors.then_some(self.instruments.node_errors.clone()),
                cfg.success_rate
                    .then_some(self.instruments.node_success.clone()),
                cfg.fail_rate.then_some(self.instruments.node_fail.clone()),
                cfg.performance.then_some(self.instruments.node_latency.clone()),
            )
        };

        NodeTelemetryHandle {
            cfg,
            #[cfg(feature = "metrics")]
            count,
            #[cfg(feature = "metrics")]
            errors,
            #[cfg(feature = "metrics")]
            success,
            #[cfg(feature = "metrics")]
            fail,
            #[cfg(feature = "metrics")]
            latency,
            #[cfg(feature = "metrics")]
            attrs,
        }
    }

    /// Build a tracing span for a graph execution.
    ///
    /// The span is only meaningful when the consumer crate enables the `trace`
    /// feature and a Tempo OTLP receiver is available at the configured endpoint.
    pub fn graph_span(&self, graph: &'static str) -> tracing::Span {
        tracing::info_span!("graphium.graph", graph = graph)
    }

    /// Build a tracing span for a node execution.
    pub fn node_span(&self, graph: &'static str, node: &'static str) -> tracing::Span {
        tracing::info_span!("graphium.node", graph = graph, node = node)
    }

    pub fn shutdown(&self) {
        #[cfg(feature = "trace")]
        {
            let _ = self.tracer_provider.shutdown();
        }
        #[cfg(feature = "metrics")]
        {
            let _ = self.meter_provider.shutdown();
        }
        #[cfg(feature = "logs")]
        {
            let _ = self.logger_provider.shutdown();
        }
    }
}

#[cfg(feature = "trace")]
fn init_traces(resource: &Resource, endpoints: &TelemetryEndpoints) -> opentelemetry_sdk::trace::SdkTracerProvider {
    use opentelemetry::global;
    use opentelemetry_otlp::{Protocol, SpanExporter, WithExportConfig};

    let exporter = SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(endpoints.tempo_otlp_http.clone())
        .build()
        .expect("build otlp span exporter");

    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource.clone())
        .build();

    global::set_tracer_provider(provider.clone());
    provider
}

#[cfg(feature = "logs")]
fn init_logs(
    resource: &Resource,
    endpoints: &TelemetryEndpoints,
) -> opentelemetry_sdk::logs::SdkLoggerProvider {
    use opentelemetry_otlp::{LogExporter, Protocol, WithExportConfig};

    let exporter = LogExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(endpoints.loki_otlp_http.clone())
        .build()
        .expect("build otlp log exporter");

    let provider = opentelemetry_sdk::logs::SdkLoggerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource.clone())
        .build();

    provider
}

#[cfg(feature = "metrics")]
fn init_metrics(
    resource: &Resource,
    endpoints: &TelemetryEndpoints,
) -> (opentelemetry_sdk::metrics::SdkMeterProvider, MetricInstruments) {
    use opentelemetry::global;
    use opentelemetry_otlp::{MetricExporter, Protocol, WithExportConfig};
    use opentelemetry_sdk::metrics::SdkMeterProvider;

    let exporter = MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(endpoints.prometheus_otlp_http.clone())
        .build()
        .expect("build otlp metric exporter");

    let provider = SdkMeterProvider::builder()
        .with_periodic_exporter(exporter)
        .with_resource(resource.clone())
        .build();

    global::set_meter_provider(provider.clone());
    let meter = global::meter("graphium");

    let instruments = MetricInstruments {
        node_count: meter.u64_counter("graphium_node_count_total").build(),
        node_count_by_caller: meter
            .u64_counter("graphium_node_count_by_caller_total")
            .build(),
        node_errors: meter.u64_counter("graphium_node_errors_total").build(),
        node_errors_by_caller: meter
            .u64_counter("graphium_node_errors_by_caller_total")
            .build(),
        node_success: meter.u64_counter("graphium_node_success_total").build(),
        node_success_by_caller: meter
            .u64_counter("graphium_node_success_by_caller_total")
            .build(),
        node_fail: meter.u64_counter("graphium_node_fail_total").build(),
        node_fail_by_caller: meter
            .u64_counter("graphium_node_fail_by_caller_total")
            .build(),
        node_latency: meter
            .f64_histogram("graphium_node_latency_seconds")
            .build(),
        node_latency_by_caller: meter
            .f64_histogram("graphium_node_latency_by_caller_seconds")
            .build(),

        graph_count: meter.u64_counter("graphium_graph_count_total").build(),
        graph_count_by_caller: meter
            .u64_counter("graphium_graph_count_by_caller_total")
            .build(),
        graph_errors: meter.u64_counter("graphium_graph_errors_total").build(),
        graph_errors_by_caller: meter
            .u64_counter("graphium_graph_errors_by_caller_total")
            .build(),
        graph_success: meter.u64_counter("graphium_graph_success_total").build(),
        graph_success_by_caller: meter
            .u64_counter("graphium_graph_success_by_caller_total")
            .build(),
        graph_fail: meter.u64_counter("graphium_graph_fail_total").build(),
        graph_fail_by_caller: meter
            .u64_counter("graphium_graph_fail_by_caller_total")
            .build(),
        graph_latency: meter
            .f64_histogram("graphium_graph_latency_seconds")
            .build(),
        graph_latency_by_caller: meter
            .f64_histogram("graphium_graph_latency_by_caller_seconds")
            .build(),
    };

    (provider, instruments)
}

#[allow(unused_variables)]
fn install_tracing_subscriber(
    #[cfg(feature = "trace")] tracer_provider: &opentelemetry_sdk::trace::SdkTracerProvider,
    #[cfg(feature = "logs")] logger_provider: &opentelemetry_sdk::logs::SdkLoggerProvider,
) {
    #[cfg(feature = "trace")]
    use opentelemetry::trace::TracerProvider;
    use tracing_subscriber::prelude::*;

    let registry = tracing_subscriber::registry();

    #[cfg(feature = "trace")]
    let registry = {
        let tracer = tracer_provider.tracer("graphium");
        registry.with(tracing_opentelemetry::layer().with_tracer(tracer))
    };

    #[cfg(feature = "logs")]
    let registry = {
        let bridge = opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
            logger_provider,
        );
        registry.with(bridge)
    };

    let _ = registry.try_init();
}
