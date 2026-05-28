use std::time::Instant;

/// Re-export so macro-generated code can use `tracing::*` without requiring
/// a direct dependency in the consumer crate.
pub use tracing;

/// Default local endpoints for exporting telemetry.
///
/// These are intended for local docker-compose / k8s port-forward setups.
/// They can be overridden with [`TelemetryEndpoints`] when telemetry is enabled.
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

impl Default for TelemetryEndpoints {
    fn default() -> Self {
        Self {
            prometheus_otlp_http: DEFAULT_PROMETHEUS_OTLP_HTTP.to_string(),
            loki_otlp_http: DEFAULT_LOKI_OTLP_HTTP.to_string(),
            tempo_otlp_http: DEFAULT_TEMPO_OTLP_HTTP.to_string(),
            service_name: "graphium".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct GraphiumTelemetry;

impl GraphiumTelemetry {
    /// No-op singleton when telemetry features are disabled.
    pub fn global() -> Self {
        GraphiumTelemetry
    }

    /// No-op.
    pub fn shutdown(&self) {}

    /// No-op start timer.
    pub fn start_timer(&self) -> Instant {
        Instant::now()
    }

    /// No-op.
    pub fn record_success(&self, _start: Instant) {}

    /// No-op.
    pub fn record_failure(&self, _start: Instant) {}

    /// No-op graph metrics accessor used by macro-generated code.
    pub fn graph_metrics(&self, _graph: &str, _caller: Option<&str>) -> GraphMetrics {
        GraphMetrics
    }

    /// No-op node metrics accessor used by macro-generated code.
    pub fn node_metrics(
        &self,
        _graph: &str,
        _node: &str,
        _caller: Option<&str>,
    ) -> NodeMetrics {
        NodeMetrics
    }
}

#[derive(Clone, Copy)]
pub struct GraphMetrics;

impl GraphMetrics {
    pub fn start_timer(&self) -> Instant {
        Instant::now()
    }
    pub fn record_success(&self, _start: Instant) {}
    pub fn record_failure(&self, _start: Instant) {}
}

#[derive(Clone, Copy)]
pub struct NodeMetrics;

impl NodeMetrics {
    pub fn start_timer(&self) -> Instant {
        Instant::now()
    }
    pub fn record_success(&self, _start: Instant) {}
    pub fn record_failure(&self, _start: Instant) {}
}
