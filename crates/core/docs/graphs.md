# `graph!` macro

`graph! { ... }` defines a **typed workflow** (a DAG) in Graphium’s DSL and expands it into plain Rust code at compile time.

**See also**

- `index.md` (documentation map)
- `getting_started.md` (first graph)
- `dsl.md` (operators, control-flow, syntax)
- `artifacts.md` (owned vs borrowed vs taken)
- `nodes.md` (how `node!` wrappers are generated)
- `async.md` (async graphs behavior)

---

## What `graph!` generates

At a high level, `graph!` expands into:

- A `pub struct MyGraph;` type (the “graph type”)
- `Default` for the graph type
- A `run(...)` method for sync graphs
- A `run_async(...)` method (always generated; used when the graph is marked `async`)
- Feature-gated helpers for visualization/export/metrics/playground (see “Feature-gated output” below)

The macro implementation is in `crates/macro/src/graph_macro/*`.

---

## Graph “levels” (low / mid / high)

Graphium can express workflows at different abstraction levels (examples adapted from the root `README.md`):

### Low-level graphs (function pipelines)

Nodes are small pure-ish transformations and the graph behaves like an explicit pipeline:

```rust
use graphium_macro::{graph, node};

node! { fn add(a: u32, b: u32) -> u32 { a + b } }
node! { fn pow(a: u32) -> u32 { a * a } }

graph! {
    TransformGraph(a: u32, b: u32) -> (out: u32) {
        Add(a, b) -> (c) >>
        Pow(c) -> (out)
    }
}
```

### Mid-level graphs (domain operations + artifacts)

Nodes become domain operations; artifacts represent domain objects flowing through the pipeline:

```rust
use graphium_macro::{graph, node};

#[derive(Default)]
pub struct Context { /* services, config, etc. */ }

node! { fn get_dataset() -> Vec<u8> { vec![1, 2, 3] } }
node! { fn preprocess(dataset: &Vec<u8>) -> usize { dataset.len() } }

graph! {
    Example<'a, Context> -> (n: usize) {
        GetDataset() -> (&'a dataset) >>
        Preprocess(&'a dataset) -> (n)
    }
}
```

### High-level graphs (orchestration)

A graph can orchestrate subsystems and call nested graphs:

```rust
use graphium_macro::{graph, node};

node! { fn setup_storage() {} }
node! { fn launch_ingestion_pipeline() {} }
node! { fn start_http_api() {} }

graph! {
    App<'a, graphium::Context> {
        SetupStorage() >>
        LaunchIngestionPipeline() &&
        StartHttpApi()
    }
}
```

---

## Syntax: the outer signature

The `graph!` signature defines:

- Whether the graph is `async`
- Optional lifetimes used by artifact references (e.g. `'a`)
- The context type (optional; defaults to `()`)
- Typed graph inputs (optional)
- Typed graph outputs (optional)

Examples:

```rust
use graphium_macro::{graph, node};

#[derive(Default)]
pub struct MyCtx;

node! { fn a() {} }
node! { fn b() {} }
node! { fn add(a: u32, b: u32) -> u32 { a + b } }
node! { fn inc(a: u32) -> u32 { a + 1 } }
node! { fn uses_ctx(ctx: &MyCtx) { let _ = ctx; } }
node! { fn get() -> u32 { 1 } }
node! { fn use_value(v: &u32) { let _ = v; } }
node! { fn noop() {} }

graph! { NoIO { A() >> B() } }
graph! { WithInputs(a: u32, b: u32) -> (out: u32) { Add(a, b) -> (out) } }
graph! { WithOutput(a: u32) -> (b: u32) { Inc(a) -> (b) } }
graph! { WithCtx<MyCtx> { UsesCtx() } }
graph! { WithLifetime<'a> { Get() -> (&'a value) >> UseValue(&'a value) } }
graph! { WithLifetimeAndCtx<'a, MyCtx> { Noop() } }
graph! { async AsyncGraph<'a, MyCtx> { Noop() } }
```

### Context injection (how nodes receive `ctx`)

If a node function declares a parameter named `ctx` or `_ctx` and its type is `&Context` or `&mut Context`,
the `node!` wrapper marks it as a context parameter and generated graphs pass the graph context automatically.

See `nodes.md` for the exact rules.

---

## Syntax: the body (nodes + control flow)

Inside the body you compose “atoms”:

- Node calls: `SomeNode(x, y) -> (out)`
- Control-flow atoms prefixed by `@`: `@match`, `@if`, `@while`, `@loop`, `@break`
- Composition operators:
  - `>>` for sequencing
  - `&&` for parallel groups

Full DSL details live in `dsl.md`.

---

## Nested graphs

In the DSL, calling another graph’s `run` is treated specially: it executes the nested graph rather than behaving like a normal node wrapper call.

Example:

```rust
use graphium_macro::{graph, node};

node! { fn inc(a: u32) -> u32 { a + 1 } }

graph! { Inner(a: u32) -> (b: u32) { Inc(a) -> (b) } }

graph! {
    Outer(a: u32) -> (b: u32) {
        Inner::run(a) -> (b)
    }
}
```

Internally, this is detected by checking whether the path ends with `::run` (see `crates/macro/src/ir.rs` `is_graph_run_path` and `graph_macro/expr/single.rs`).

---

## Feature-gated output

`graph!` always emits the runnable graph type. Additional helper impls are feature-gated:

- `metrics`: wraps `run` / `run_async` bodies with metrics instrumentation
- `dto` and `export`: emit DTO/metadata used by `graphium-ui` and other tooling
- `playground`: emit helper impls for interactive execution (used by the UI)

The code paths are in `crates/macro/src/graph_macro/expand/*`.

---

## How `graph!` works internally (accurate mental model)

This is the “what the macro does” overview. For the exact code, start at:

- `crates/macro/src/graph_macro/expand/mod.rs` (top-level expander)
- `crates/macro/src/graph_macro/parse.rs` (DSL parsing into IR)
- `crates/macro/src/graph_macro/analysis.rs` (static “shape” analysis)
- `crates/macro/src/graph_macro/expr/*` (codegen for each expression kind)
- `crates/macro/src/graph_macro/execution.rs` (build `run` / `run_async` bodies)

### 1) Parse: tokens → IR

`graph!` input is parsed into a typed internal representation:

- `GraphInput`: outer signature + attributes + the body expression tree
- `NodeExpr`: the body as an expression tree (`Sequence`, `Parallel`, `Route`, `While`, `Loop`, `Single`, `Break`)
- `NodeCall`: a node call with:
  - artifact input names + “kind” (`Owned`/`Borrowed`/`Taken`)
  - artifact output names + optional borrow spec (`None` vs `Some(&...)`)

These types live in `crates/macro/src/ir.rs`.

### 2) Analyze: compute “shape” for validation

Before generating code, the macro computes the “shape” of expressions:

- Which artifacts are required to enter an expression
- Which artifacts may leave an expression

This enables validations such as:

- Route branches must agree with declared outputs
- Loop bodies must produce exactly what the loop claims to output
- Parallel groups must not produce duplicate artifact names

The analyzer lives in `crates/macro/src/graph_macro/analysis.rs` and is used by route/loop codegen.

### 3) Expand: IR → hop-by-hop Rust

Codegen expands the `NodeExpr` tree into a **hop-based orchestration**:

- “Owned” artifacts flow as `Option<T>` locals in a hop payload.
- When an owned artifact is consumed once, codegen does `.take()` (move).
- When it is consumed multiple times in the same step, codegen clones it via `graphium::clone_artifact(&value)` (fan-out).

Borrowed artifacts are different:

- Persisted artifacts are stored in graph-local “borrowed slots”
  (locals like `let mut __graphium_borrowed_number: Option<T> = None;`).
- `&artifact`/`&mut artifact` reads from those slots via `.as_ref()` / `.as_mut()`.
- `*artifact` takes the owned value out of the slot via `.take()`.

This is implemented in `crates/macro/src/graph_macro/expr/single.rs` and related `expr/*` modules.

### 4) Wrap into `run` / `run_async`

Finally, `execution.rs`:

- Turns graph inputs into stable function parameters
- Seeds the “root payload” for the expression expander
- Emits the borrowed-slot locals needed for the whole graph execution
- Builds the return expression for declared graph outputs

If the graph is declared `async`, the sync `run` body is intentionally not generated (only `run_async` is meaningful).
