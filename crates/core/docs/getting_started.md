# Getting started

This is a practical “first steps” guide for using Graphium in your own crate.

Recommended reading order after this:

0. `index.md` — documentation map
1. `dsl.md`
2. `artifacts.md`
3. `graphs.md`
4. `nodes.md`
5. `async.md` / `control_flow.md` / `telemetry.md` (as needed)

---

## 1) Add the dependency

To use the `graph!` / `node!` macros from the `graphium` crate, enable the `macros` feature:

```toml
[dependencies]
graphium = { version = "0.1", features = ["macros"] }
```

You can also import macros directly from the macro crate:

```rust
use graphium_macro::{graph, node};
```

---

## 2) Define a node

Nodes are normal Rust functions wrapped by `node!`:

```rust
use graphium_macro::node;

node! {
    fn get_number() -> u32 {
        42
    }
}
```

Rules to remember:

- Node return types must be owned (no `&T` returns).
- Node inputs can be `T`, `&T`, or `&mut T`.
- A parameter named `ctx` / `_ctx` becomes the graph context only when it is `&Context` or `&mut Context`.

See `nodes.md`.

---

## 3) Define a graph

Graphs are defined with `graph!` and composed from node calls:

```rust
use graphium_macro::{graph, node};

node! { fn get_number() -> u32 { 42 } }
node! { fn inc(n: u32) -> u32 { n + 1 } }

graph! {
    MyGraph -> (out: u32) {
        GetNumber() -> (n) >>
        Inc(n) -> (out)
    }
}
```

Run it:

```rust
# use graphium_macro::{graph, node};
# node! { fn get_number() -> u32 { 42 } }
# node! { fn inc(n: u32) -> u32 { n + 1 } }
# graph! {
#     MyGraph -> (out: u32) {
#         GetNumber() -> (n) >>
#         Inc(n) -> (out)
#     }
# }
let mut ctx = graphium::Context::default();
let out = MyGraph::run(&mut ctx);
assert_eq!(out, 43);
```

See `graphs.md` for signature options (inputs/outputs/context/lifetimes/async).

---

## 4) Understand artifacts (critical)

Artifacts are the named values in the DSL (like `n` and `out` above).

You can:

- Move owned values (`n`)
- Persist values into graph storage (`-> (&n)` / `-> (&mut n)`)
- Borrow persisted values (`&n`, `&mut n`)
- Take persisted values out (`*n`, `*mut n`)

See `artifacts.md`.

---

## 5) Enable optional tooling/features

Graphium is designed so the core can be “zero-cost-ish” by default, and observability/tooling can be enabled via feature flags:

- `metrics`, `trace`, `logs` (OpenTelemetry via OTLP/HTTP)
- `dto` / `export` (metadata export for UI/tools)
- `playground` (generate UI runnable playground helpers)

See `features.md` and `telemetry.md`.

---

## Common compilation gotchas

- “node return type must be owned (no references)”
  - Fix: return an owned type from the node and persist it in the graph via `-> (&artifact)` if you need downstream borrows.

- “missing borrowed artifact `...` for node call”
  - Fix: you tried to use `&artifact` / `*artifact` without first persisting `artifact` into graph storage via a borrowed output.

- “parallel step produces duplicate artifact `...`”
  - Fix: two branches of the same `&&` group produced the same output artifact name.

- “`@break` can only be used inside `@loop` or `@while`”
  - Fix: move the `@break` into a loop body.
