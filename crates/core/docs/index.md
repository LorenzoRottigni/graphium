# Graphium documentation

This folder contains the “source of truth” documentation for Graphium’s core concepts and optional tooling.

## Recommended reading order

1. `getting_started.md` — install + first graph
2. `dsl.md` — DSL operators + syntax
3. `artifacts.md` — ownership model (owned / borrowed / taken)
4. `nodes.md` — `node!` signatures and expansion model
5. `graphs.md` — `graph!` signatures, nesting, internals
6. `control_flow.md` — practical patterns for branching/loops
7. `async.md` — async graphs and current parallelism behavior
8. `features.md` — feature flags and tradeoffs
9. `telemetry.md` — metrics/traces/logs (Grafana stack)
10. `testing.md` — test macros + UI runner
11. `dashboard.md` — Graphium UI dashboard and playground

## Scope of each document

- `getting_started.md`: end-to-end “hello graph” and common pitfalls
- `dsl.md`: operators (`>>`, `&&`), control-flow atoms (`@...`), precedence, and where each rule lives in the codebase
- `artifacts.md`: what artifacts are + persistence/borrows/takes + fan-out cloning rules
- `nodes.md`: how `node!` wraps functions (`run`/`run_async`), signature rules, and internal pipeline
- `graphs.md`: how `graph!` turns DSL into executable Rust, nesting, feature-gated outputs, and internals map
- `control_flow.md`: examples and best practices for `@match`/`@if`/loops + artifact interactions
- `async.md`: differences vs sync graphs (including current `&&` behavior)
- `features.md`: feature-by-feature explanation (`macros`, `dto`, `export`, `playground`, `metrics`, `trace`, `logs`)
- `telemetry.md`: OTLP endpoints, env vars, metric names/labels, dashboard queries
- `testing.md`: `graph_test!`/`node_test!`, `#[tests(...)]`, UI discoverability, limitations
- `dashboard.md`: UI pages, fragment rendering, data sources, playground constraints and overhead
