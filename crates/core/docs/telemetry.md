# Telemetry (metrics, traces, logs)

Graphium’s runtime telemetry is implemented in `graphium::telemetry` (`crates/core/src/telemetry.rs`) and is activated by feature flags.

Graphium uses **OpenTelemetry** exporters over **OTLP/HTTP**:

- Metrics → Prometheus OTLP HTTP receiver
- Logs → Loki OTLP HTTP receiver
- Traces → Tempo OTLP HTTP receiver

The Graphium UI dashboard (`crates/ui`) can then *query* those backends over their normal HTTP APIs.

**See also**

- `index.md` (documentation map)
- `features.md` (what enabling `metrics`/`trace`/`logs` does)
- `dashboard.md` (how the UI queries Prometheus/Loki/Tempo)

---

## Feature flags (what gets compiled in)

All telemetry code is feature-gated:

- `metrics`: emits OpenTelemetry metrics
- `trace`: emits OpenTelemetry spans via `tracing` → OTLP exporter
- `logs`: exports `tracing` logs via an OpenTelemetry logs exporter

These are `graphium` crate features (see `crates/core/Cargo.toml`).

Important: even if you enable `metrics`/`trace`/`logs`, you still need a backend running at the configured endpoints to actually receive anything.

---

## Endpoints and environment variables

Graphium centralizes endpoint configuration in `TelemetryEndpoints` and supports common OpenTelemetry env vars.

Defaults (intended for local docker-compose / port-forward setups):

- Prometheus metrics OTLP: `http://127.0.0.1:9090/api/v1/otlp/v1/metrics`
- Loki logs OTLP: `http://127.0.0.1:3100/otlp/v1/logs`
- Tempo traces OTLP: `http://127.0.0.1:4318/v1/traces`

Override precedence (highest first):

- `GRAPHIUM_PROMETHEUS_OTLP_HTTP`, or `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT`, or `OTEL_EXPORTER_OTLP_ENDPOINT + "/v1/metrics"`
- `GRAPHIUM_LOKI_OTLP_HTTP`, or `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT`, or `OTEL_EXPORTER_OTLP_ENDPOINT + "/v1/logs"`
- `GRAPHIUM_TEMPO_OTLP_HTTP`, or `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`, or `OTEL_EXPORTER_OTLP_ENDPOINT + "/v1/traces"`

Service name:

- `GRAPHIUM_SERVICE_NAME`, or `OTEL_SERVICE_NAME`, default `"graphium"`

See `crates/core/src/telemetry.rs` (`TelemetryEndpoints::from_env`).

---

## How telemetry is initialized

Macro-generated graph code calls `graphium::GraphiumTelemetry::global()` when any of `metrics`, `trace`, or `logs` are enabled.

`GraphiumTelemetry::global()`:

- Initializes providers once (via `OnceLock`)
- Sets OpenTelemetry global providers (meter/tracer where enabled)
- Installs a `tracing_subscriber` registry:
  - trace layer when `trace` is enabled
  - OpenTelemetry log bridge when `logs` is enabled

This makes telemetry “just work” once your binary first runs an instrumented graph/node, without manual setup code.

See `crates/core/src/telemetry.rs` (`GraphiumTelemetry::{global, init_global, init}`).

---

## Metrics: names and labels

Graphium defines a small set of standard metrics (counters + histograms).
Metric emission is controlled by the legacy `#[metrics(...)]` config compiled into macro-generated wrappers.

### Graph metrics

- `graphium_graph_count_total`
- `graphium_graph_errors_total`
- `graphium_graph_success_total`
- `graphium_graph_fail_total`
- `graphium_graph_latency_seconds` (histogram)

Optional `*_by_caller_*` variants exist when the “caller” dimension is enabled:

- `graphium_graph_count_by_caller_total`
- `graphium_graph_latency_by_caller_seconds`
- etc.

Labels used by Graphium:

- `graph` (graph type name)
- `caller` (usually `module_path!()`), only when enabled

### Node metrics

- `graphium_node_count_total`
- `graphium_node_errors_total`
- `graphium_node_success_total`
- `graphium_node_fail_total`
- `graphium_node_latency_seconds` (histogram)

Labels:

- `graph`
- `node`
- `caller` (optional)

Metric instruments are created in `crates/core/src/telemetry.rs` (`init_metrics`).

---

## Traces and spans

When the `trace` feature is enabled:

- `graph!`-generated `run`/`run_async` enters a span named `graphium.graph` with field `graph = <GraphName>`
- Node wrappers can similarly enter `graphium.node` spans (graph + node fields)

Span exporting is wired via OTLP/HTTP to the Tempo endpoint.

See:

- `crates/core/src/telemetry.rs` (`graph_span`, `node_span`, `init_traces`)
- `crates/macro/src/graph_macro/expand/runtime/{sync,async}.rs` (span entry in run methods)

---

## Logs (Loki via OTLP)

When the `logs` feature is enabled:

- Graph runners emit `tracing::info!(graph = "...", "graph started")` and `"graph finished"`
- Tracing events are bridged into OpenTelemetry logs and exported via OTLP/HTTP to the Loki endpoint

See:

- `crates/core/src/telemetry.rs` (`init_logs`, `install_tracing_subscriber`)
- `crates/macro/src/graph_macro/expand/runtime/{sync,async}.rs` (start/finish logs)

---

## How the dashboard queries telemetry backends

Graphium UI uses normal HTTP query APIs (not OTLP):

### Prometheus queries

UI calls: `GET /api/v1/query?query=...` on `prometheus_url`.

Used queries include:

- `sum(graphium_graph_count_total{graph="..."})`
- `histogram_quantile(0.95, sum(rate(graphium_graph_latency_seconds_bucket{graph="..."}[5m])) by (le))`

See `crates/ui/src/metrics.rs`.

### Loki queries

UI calls: `GET /loki/api/v1/query_range` on `loki_url` (last 30 minutes, backward).

The UI filters by labels like:

- `{service_name="graphium",graph="..."}`
- `{service_name="graphium",graph="...",node="..."}`

See `crates/ui/src/logs.rs`.

### Tempo queries

UI calls:

- `GET /api/search?q=...&limit=...` on `tempo_url`
- then links to `GET /api/v2/traces/<trace_id>`

The UI’s search query is:

- `{ .service.name = "graphium" && .graph = "..." }` (and `&& .node = "..."` for node views)

See `crates/ui/src/traces.rs`.

---

## Practical setup notes (Grafana stack)

The dashboard expects three distinct concepts to work at once:

1. OTLP receivers that accept what Graphium exports (metrics/logs/traces)
2. Storage/query APIs (Prometheus/Loki/Tempo HTTP APIs)
3. Grafana as a UI that can visualize those backends (optional, but recommended)

Graphium itself does not ship a compose file in `crates/core/docs`; the defaults are chosen to match common local setups.
