# GRAPHIUM

<img src="https://s3.rottigni.tech/public/github/graphium/graphium_logo.png" alt="graphium" width="250px" height="250px" />

Graphium is a Rust framework for defining **observable DAG-based workflows** through a declarative, Rust-friendly DSL.

It allows developers to express computation as composable algorithmic graphs, where logic is modeled as directed acyclic graphs (DAGs) and expanded into Rust code at compile time.

The framework focuses on **zero-cost abstractions**: DSL parsing and graph expansion happen entirely at compile time, and the generated code is further optimized by LLVM.

Runtime behavior is optional and controlled via feature flags. Using Graphium with only the `"macros"` feature results in near-zero runtime overhead, since all orchestration is resolved at compile time.

## Documentation

Start here: `crates/core/docs/index.md`.

---

## Install

```toml
[dependencies]
graphium = { version = "0.1", features = ["macros"] }
```

Enable additional features (optional): `dto`, `export`, `playground`, `metrics`, `trace`, `logs`.

---

## Why Graphium

- **Typed dataflow**: values are explicit artifacts flowing between nodes.
- **Ownership-aware DSL**: owned / borrowed / taken artifacts map to safe Rust semantics.
- **Observable** (optional): metrics, traces, and logs via OpenTelemetry.
- **Tooling-friendly** (optional): export DTOs for visualization and UI execution.

---

## Quick taste

Graphium graphs are written as a schema and compiled into Rust code:

```rust
use graphium::{graph, node};

#[derive(Default)]
pub struct Context {}

node! { fn get_number() -> u32 { 42 } }
node! { fn duplicate(a: u32) -> (u32, u32) { (a, a) } }
node! { fn pipe(a: u32) -> u32 { a } }

graph! {
    #[metrics("performance", "errors", "count")]
    ExampleGraph<Context> -> (out: u32) {
        GetNumber() -> (number) >>
        Duplicate(number) -> (left, right) >>
        Pipe(left) -> (out)
    }
}
```

---

## Use cases (low / mid / high level)

Graphium can model workflows at different levels of abstraction:

### Low-level (function pipeline)

```rust
use graphium::{graph, node};

node! { fn add(a: u32, b: u32) -> u32 { a + b } }
node! { fn pow(a: u32) -> u32 { a * a } }

graph! {
    TransformGraph<graphium::Context>(a: u32, b: u32) -> (out: u32) {
        Add(a, b) -> (c) >>
        Pow(c) -> (out)
    }
}
```

### Mid-level (domain operations + artifacts)

```rust
use graphium::{graph, node};

#[derive(Default)]
pub struct Context { pub threshold: u32 }

node! { fn get_threshold(ctx: &Context) -> u32 { ctx.threshold } }
node! { fn compute() -> u32 { 10 } }
node! { fn is_ok(v: u32, threshold: u32) -> bool { v >= threshold } }

graph! {
    RuleGraph<Context> -> (ok: bool) {
        GetThreshold() -> (threshold) >>
        Compute() -> (value) >>
        IsOk(value, threshold) -> (ok)
    }
}
```

### High-level (orchestration + nested graphs)

```rust
use graphium::{graph, node};

node! { fn setup_storage() {} }
node! { fn start_http_api() {} }

graph! {
    Platform<graphium::Context> {
        SetupStorage() >>
        StartHttpApi()
    }
}
```

For in-depth DSL rules and artifact ownership, see `crates/core/docs/index.md`.

## Graphium UI

<img src="https://s3.rottigni.tech/public/github/graphium/graphium_graph_hero.png" alt="graphium" width="800px" />

Graphium ships a local UI for visualization, playground execution, and test runs. See `crates/ui/README.md`.
