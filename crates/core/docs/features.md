# Feature flags

Graphium is intentionally split into:

- A compile-time DSL (`graph!`, `node!`) that generates Rust code
- Optional runtime/tooling support that you enable via feature flags

This document describes what each feature flag does in the `graphium` crate (`crates/core`) and how it affects generated code, runtime overhead, and UI/dashboard support.

**See also**

- `index.md` (documentation map)
- `telemetry.md` (metrics/traces/logs details)
- `dashboard.md` (UI usage, playground constraints)

---

## Where features are defined

`graphium` crate features are defined in `crates/core/Cargo.toml`.

Key ones:

- `macros`
- `dto`
- `export` (serde-enabled export)
- `metrics`
- `trace`
- `logs`
- `playground`
- `serialize` (deprecated alias of `export`)

---

## `macros`

Enables the procedural macro dependency and re-exports:

- `graph!`
- `node!`
- `graph_test!`
- `node_test!`

Implementation note:

- With `macros` enabled, `graphium` depends on `graphium-macro`.
- Without it, you can still depend on `graphium` as a runtime crate, but you cannot use the DSL from `graphium::{graph, node}`.

---

## `dto`

Enables macro-generated metadata helpers that do **not** require serde.

This is used by tooling (notably `graphium-ui`) to obtain:

- Graph schema / node schema representations
- Flow information for visualization
- Docs/tags/deprecation metadata (when present)

The macro expander emits DTO-related impls in `crates/macro/src/graph_macro/expand/metadata.rs` and friends.

Overhead profile:

- Compile time: more codegen + more generated tokens
- Binary size: increased (more static metadata)
- Runtime: mostly none unless you call DTO helpers or the UI uses them

---

## `export`

`export` is “DTO + serde”.

It enables:

- `graphium::serde` re-export (for consumers)
- serde derives on exported DTO types (e.g. `GraphDto`, `GraphiumBundleDto`, etc.)

This is typically required when you want to serialize graph metadata to JSON for:

- External UIs
- Persisted snapshots
- Remote inspection

Overhead profile:

- Compile time: more (serde derives)
- Binary size: more (serde code + metadata)
- Runtime: serialization cost when you actually serialize DTOs

---

## `playground`

Enables macro-generated “playground” helpers for graphs/nodes that allow the UI to:

- Render input forms based on a schema (`GraphPlayground::playground_schema()`)
- Execute a graph/node from `HashMap<String, String>` form values (`playground_run`)

Playground support is intentionally conservative:

- Graph playground is supported only for **sync graphs** whose inputs are parseable from strings (`String`, `bool`, numeric primitives).
- Playground execution uses `Context: Default` and runs inside the UI process.

Code paths:

- `crates/macro/src/graph_macro/expand/playground.rs`
- `crates/macro/src/node_macro/expand.rs` (node playground helpers)

Overhead profile:

- Compile time: some additional codegen
- Runtime: only when calling playground methods; UI also incurs HTTP + render overhead

See `dashboard.md`.

---

## `metrics`

Enables OpenTelemetry metrics emission for graphs and nodes via `graphium::telemetry`.

What you get:

- Standard counters/histograms for graph and node execution
- Optional dimensions like “caller” when enabled by `#[metrics(...)]`

How it’s wired:

- Macro-generated code calls `graphium::GraphiumTelemetry::global()` lazily.
- Instruments are created once and reused via static `OnceLock` handles.

Metric names are defined in `crates/core/src/telemetry.rs` (`init_metrics`), and the dashboard queries these from Prometheus (`crates/ui/src/metrics.rs`).

Overhead profile:

- Per execution: counter increments + optional histogram recording
- Export: periodic OTLP metric export (via OpenTelemetry SDK periodic reader)

See `telemetry.md`.

---

## `trace`

Enables OpenTelemetry tracing via `tracing` spans exported via OTLP/HTTP.

What you get:

- A `graphium.graph` span around each graph execution (`run`/`run_async`)
- Node spans where wrappers emit them (when node tracing is enabled)

How it’s wired:

- `GraphiumTelemetry` initializes an OTLP span exporter to Tempo.
- A `tracing_subscriber` layer bridges `tracing` spans to OpenTelemetry.

Overhead profile:

- Per execution: span creation/enter/exit, plus batch exporting

See `telemetry.md`.

---

## `logs`

Enables OpenTelemetry log export from `tracing` events.

What you get:

- Graph start/finish logs emitted by macro-generated graph runners
- Logs exported via OTLP/HTTP to Loki

How it’s wired:

- `GraphiumTelemetry` initializes an OTLP log exporter.
- `install_tracing_subscriber` installs an OpenTelemetry log bridge layer.

Overhead profile:

- Per log event: event formatting + export pipeline costs

See `telemetry.md`.

---

## `serialize` (deprecated)

Alias of `export`. Prefer using `export` going forward.

---

## Choosing features (suggested presets)

Minimal (compile-time DSL only):

- `macros`

Local UI (visualize + run simple playgrounds):

- `macros`, `dto`, `playground`

Full observability (Grafana stack):

- `macros`, `metrics`, `trace`, `logs`

Metadata export over the wire:

- `macros`, `export` (and usually `dto`)
