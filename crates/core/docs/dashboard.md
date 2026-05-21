# Graphium UI Dashboard

This document explains how the Graphium UI “dashboard” works: what it renders, how it fetches data, what needs to be enabled in your graphs, and what overheads to expect.

The “dashboard” lives in the `graphium-ui` crate (`crates/ui`) and is a local Axum server that renders server-side HTML (Askama templates) with HTMX/Alpine.js on the client.

---

## What the dashboard is

The dashboard is the Graphium UI page that lets you:

- Select a configured graph and visualize it (Mermaid)
- Inspect the raw schema used to generate the graph
- Inspect docs/tags/deprecation metadata (when exported)
- Browse linked nodes and tests
- View recent metrics, logs, and traces for the selected graph (via external backends)
- Run the graph “playground” when available (execute a graph from a form)

Routes of interest:

- `/dashboard`: loads the default graph (first configured)
- `/graph/:id`: dashboard view for a specific graph
- `/fragment/graph/:id`: HTMX fragment endpoint used to re-render only the graph panel
- `/graph/:id/playground/run`: executes the playground run and returns an updated fragment

See `crates/ui/src/server.rs`.

---

## How rendering works (page + fragment)

The UI uses a 2-step render strategy:

1. `dashboard.html` provides the “shell” (layout + graph selector).
2. The main content area loads `graph_fragment.html` via HTMX (`hx-get` on page load), and subsequent graph selections swap that fragment.

Mermaid is loaded client-side from a CDN, and the UI re-runs Mermaid rendering after each HTMX swap.

Files:

- `crates/ui/templates/pages/dashboard.html`
- `crates/ui/templates/pages/graph_fragment.html`
- `crates/ui/src/pages/graph.rs` (`render_graph_fragment`)

---

## Data sources shown in the dashboard

### 1) Graph structure (Mermaid)

The diagram is generated from the graph export DTO (`graph.export`) and rendered as Mermaid text.

- The DTO comes from the `graph!` macro’s export support.
- The diagram renderer can optionally show artifact flow/ownership annotations.

The fragment endpoint supports a query flag to hide artifacts:

- `/fragment/graph/:id?artifacts=0` (or `false`/`off`/`no`) hides artifact rendering

See `crates/ui/src/server.rs` (`GraphVizQuery`).

### 2) Metrics cards

The dashboard shows a small set of metric “cards” (count/errors/success/fail/p50/p95).
These are fetched at request time from the configured Prometheus base URL.

Implementation entrypoint:

- `crates/ui/src/metrics.rs` (fetch + formatting)

### 3) Logs

Logs are fetched at request time from the configured Loki base URL.

Implementation entrypoint:

- `crates/ui/src/logs.rs`

### 4) Traces

Traces are fetched at request time from the configured Tempo base URL.

Implementation entrypoint:

- `crates/ui/src/traces.rs`

---

## Playground (manual execution)

The dashboard can render a “playground” form for a graph and execute it.

### When a graph has a playground

The UI shows a playground section only if the graph was configured with playground support and the `GraphPlayground` trait impl is available.

The `graph!` macro can generate a `GraphPlayground` impl under the `playground` feature flag (see `crates/macro/src/graph_macro/expand/playground.rs`).

Playground support for a graph is considered “supported” only when:

- The graph is **not async**
- All graph input types are parseable from strings (`String`, `bool`, numeric primitives)

When supported, `playground_run`:

- Builds a default context: `let mut ctx: Context = Default::default();`
- Parses form inputs into typed values
- Calls `Graph::run(&mut ctx, ...)`
- Returns `Ok(format!("{:?}", result))` (or `"ok"` if the graph returns nothing)

### Overheads and limitations of playground runs

Playground execution is intentionally “developer ergonomics first”, not production-grade:

- Uses `Default::default()` context (no custom services unless they are default-constructible)
- Parses only a limited set of scalar input types
- Serializes outputs with `Debug` formatting (not structured)
- Runs the graph inside the UI process (same address space, same CPU/memory constraints)

If you need realistic contexts (DB connections, clients, config), prefer:

- Running the graph in your application with your real context
- Using the dashboard only for visualization/inspection, or extending playground support to accept richer context configuration

---

## What “overhead” means for the dashboard

The dashboard itself does not change your graph execution unless you enable features that instrument graphs/nodes.

Typical sources of overhead when you enable dashboard-related features:

- `metrics` feature: adds wrappers that record counters/timings
- `trace` / `logs` features: emits OpenTelemetry signals
- `export` / `dto` features: increases compile-time work and binary size due to extra metadata, but does not necessarily add per-run overhead unless queried/serialized at runtime
- `playground` feature: generates parsing helpers and exposes schemas; runtime overhead mainly happens only when you actually run playground requests

The dashboard additionally performs network requests to Prometheus/Loki/Tempo when you open a graph page or refresh a fragment.

---

## Configuration (Prometheus/Loki/Tempo URLs + graphs list)

The UI is configured through `GraphiumUiConfig` and `build_state(...)` which wires:

- `prometheus_url`
- `loki_url`
- `tempo_url`
- the list of graphs to expose in the UI

See:

- `crates/ui/src/config.rs`
- `crates/ui/src/state/build.rs`

