# Async graphs

Graphium supports async graphs through the `async` keyword on `graph!` definitions.

This document describes what async graphs mean *in the current implementation* and how they differ from sync graphs.

Related docs:

- `graphs.md` for the outer `graph!` signature and generated `run`/`run_async`
- `dsl.md` for `>>`, `&&`, and control-flow atoms
- `nodes.md` for `node!` wrappers (`run` vs `run_async`)

---

## What changes when you write `async graph!`

When a graph is declared `async`:

- The macro generates `run_async(...)` (always generated for every graph type).
- The macro **does not generate** a sync `run(...)` for that graph.

This is implemented in:

- `crates/macro/src/graph_macro/expand/runtime/sync.rs` (omits `run` when `async_enabled`)
- `crates/macro/src/graph_macro/expand/runtime/async.rs` (always emits `run_async`)

Practical consequence: callers must use `MyGraph::run_async(&mut ctx, ...)` for async graphs.

---

## How nodes are called in async graphs

Inside an async graph, node calls expand to `NodeWrapper::run_async(...).await` (or nested graph `OtherGraph::run_async(...).await`).

This is implemented in:

- `crates/macro/src/graph_macro/expr/single.rs` (`node_run_call_tokens`)

Notes:

- A node can be sync or async; the wrapper API exists for both.
- If you want a node to actually perform async work, it must be defined as `async fn ...` inside `node! { ... }`.

---

## Parallel groups (`&&`) in async graphs (current behavior)

In sync graphs, Graphium can sometimes execute `A() && B()` in parallel using `std::thread::scope(...)` (subject to restrictions like “no borrowed artifacts”).

In async graphs, **parallel groups currently expand sequentially**.

This is not a documentation choice—it’s what the codegen does today:

- In `crates/macro/src/graph_macro/expr/parallel.rs`, `get_parallel_nodes_expr(...)` immediately falls back to `get_parallel_nodes_expr_sequential(...)` when `async_mode` is true.

So, in async graphs:

- `A() && B()` is parsed as a parallel group in the IR
- but is expanded as “run A, then run B” (in that order) in the generated async function

If you want true async concurrency today, you need to model it inside nodes (e.g., a node that uses `tokio::join!`) or extend the macro to emit join-based async fan-out.

---

## Async + control flow (routes and loops)

Async graphs support the same control-flow atoms as sync graphs:

- `@match`, `@if` routes
- `@while`, `@loop`, `@break`

The syntax and validation rules are the same; see `dsl.md`.

One caveat to keep in mind:

- Because async graphs do not thread-parallelize `&&`, there is no “threaded parallel inside a loop” behavior to reason about in async mode; parallel groups behave sequentially.

---

## Telemetry in async graphs

When telemetry features are enabled, `run_async`:

- Enters a `graphium.graph` span (when `trace` is enabled)
- Emits “graph started/finished” logs (when `logs` is enabled)
- Records metrics success/timing (when `metrics` is enabled)

See:

- `crates/macro/src/graph_macro/expand/runtime/async.rs`
- `crates/core/docs/telemetry.md`

---

## Do loops/branches/ifs need their own doc?

Right now:

- `dsl.md` documents the syntax, precedence, and where validation/codegen lives.

If you want “exhaustive” documentation (many examples + gotchas + patterns), it’s worth adding a dedicated `control_flow.md` later, but it doesn’t need to be separate immediately unless `dsl.md` becomes too long.

