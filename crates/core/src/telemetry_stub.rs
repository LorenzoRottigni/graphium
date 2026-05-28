use std::{sync::OnceLock, time::Instant};

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
    pub fn global() -> &'static Self {
        static TELEMETRY: OnceLock<GraphiumTelemetry> = OnceLock::new();
        TELEMETRY.get_or_init(|| GraphiumTelemetry)
    }

    /// No-op.
    pub fn shutdown(&self) {}

    /// No-op graph metrics accessor used by macro-generated code.
    pub fn graph_metrics(
        &'static self,
        _graph: &'static str,
        _caller: &'static str,
        cfg: MetricConfig,
    ) -> GraphTelemetryHandle {
        GraphTelemetryHandle { cfg }
    }

    /// No-op node metrics accessor used by macro-generated code.
    pub fn node_metrics(
        &'static self,
        _graph: &'static str,
        _node: &'static str,
        _caller: &'static str,
        cfg: MetricConfig,
    ) -> NodeTelemetryHandle {
        NodeTelemetryHandle { cfg }
    }

    /// No-op span used by macro-generated tracing.
    pub fn graph_span(&self, _graph: &'static str) -> tracing::Span {
        tracing::Span::none()
    }

    /// No-op span used by macro-generated tracing.
    pub fn node_span(&self, _graph: &'static str, _node: &'static str) -> tracing::Span {
        tracing::Span::none()
    }
}

/// Mirror of the real `MetricConfig` API so macro-generated code compiles even
/// when telemetry is disabled.
#[derive(Clone, Copy, Debug, Default)]
pub struct MetricConfig {
    pub performance: bool,
    pub errors: bool,
    pub count: bool,
    pub caller: bool,
    pub success_rate: bool,
    pub fail_rate: bool,
}

pub struct GraphTelemetryHandle {
    cfg: MetricConfig,
}

impl GraphTelemetryHandle {
    pub fn start_timer(&self) -> Option<Instant> {
        self.cfg.performance.then_some(Instant::now())
    }

    pub fn record_success(&self, _start: Option<Instant>) {}

    pub fn record_failure(&self, _start: Option<Instant>) {}
}

pub struct NodeTelemetryHandle {
    cfg: MetricConfig,
}

impl NodeTelemetryHandle {
    pub fn start_timer(&self) -> Option<Instant> {
        self.cfg.performance.then_some(Instant::now())
    }

    pub fn record_success(&self, _start: Option<Instant>) {}

    pub fn record_failure(&self, _start: Option<Instant>) {}
}
