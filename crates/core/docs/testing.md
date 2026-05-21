# Testing

Graphium supports testing in two complementary ways:

1. Normal Rust tests (using `#[test]`, `cargo test`, etc.)
2. UI-discoverable tests that can be listed and executed from Graphium UI

This document explains the macros and wiring used for (2), while still keeping your tests idiomatic for (1).

Related docs:

- `index.md` (documentation map)
- `dashboard.md` (UI test runner surfaces these tests)
- `features.md` (`dto`/`export` affects which metadata is available)
- `graphs.md` / `nodes.md` (how `#[tests(...)]` is attached to graphs/nodes)

---

## `graph_test!` and `node_test!`

Graphium provides two helper macros:

- `graph_test! { ... }`
- `node_test! { ... }`

They do two jobs at once:

- Forward normal Rust test items unchanged (so `cargo test` works as usual)
- Synthesize extra “marker” items so UI tooling can discover and execute tests

Implementation:

- `crates/macro/src/test_macro/*`

### Basic usage

```rust
#![allow(dead_code)]
use graphium_macro::{graph, graph_test, node, node_test};

#[derive(Default)]
pub struct Context;

node! { fn seven() -> u32 { 7 } }
node! { fn my_node(x: u32) -> u32 { x } }
node! { fn inc(x: u32) -> u32 { x + 1 } }

graph! {
    MyGraph<Context> -> (out: u32) {
        Seven() -> (x) >>
        MyNode(x) -> (x) >>
        Inc(x) -> (out)
    }
}

node_test! {
    #[test]
    fn my_node_works() {
        let out = MyNode::run(&(), 7);
        assert_eq!(out, 7);
    }
}

graph_test! {
    #[test]
    fn my_graph_works() {
        let mut ctx = Context::default();
        let out = MyGraph::run(&mut ctx);
        assert!(out > 0);
    }
}

fn main() {}
```

### Injected `graph` / `node` parameters (UI ergonomics)

The test macros recognize an optional first parameter:

- `graph: &MyGraph` / `graph: &mut MyGraph`
- `node: &MyNode` / `node: &mut MyNode`

This parameter is “injected” by the UI runner (and default-constructed by the macro-generated wrapper).

Example (from the examples crate):

```rust
#![allow(dead_code)]
use graphium_macro::{graph, graph_test, node};

#[derive(Default)]
pub struct Context;

node! { fn out() -> u32 { 42 } }

graph! { OwnedGraph<Context> -> (out: u32) { Out() -> (out) } }

graph_test! {
    #[test]
    fn owned_graph_returns_non_zero_split(graph: &OwnedGraph, threshold: u32) {
        let mut ctx = Context::default();
        let out = OwnedGraph::run(&mut ctx);
        assert!(out > threshold);
    }
}

fn main() {}
```

The remaining parameters (like `threshold: u32`) become UI inputs. The test macro infers basic input kinds (text/number/bool).

---

## Linking tests to graphs and nodes (`#[tests(...)]`)

To associate tests with a graph or node so they show up in Graphium UI:

- Wrap tests in `graph_test!` / `node_test!`
- Reference the generated marker types via `#[tests(...)]` on the `graph!` / `node!` definition

### Graph example

```rust
#![allow(dead_code)]
use graphium_macro::{graph, graph_test, node};

#[derive(Default)]
pub struct Context;

node! { fn out() -> u32 { 1 } }

graph_test! {
    #[test]
    fn my_graph_smoke_test() {
        let mut ctx = Context::default();
        let _ = MyGraph::run(&mut ctx);
    }
}

graph! {
    #[tests(MyGraphSmokeTest)]
    MyGraph<Context> {
        Out() -> (out)
    }
}

fn main() {}
```

### Node example

```rust
#![allow(dead_code)]
use graphium_macro::{node, node_test};

node_test! {
    #[test]
    fn my_node_smoke_test() {
        let _ = MyNode::run(&(), 1u32);
    }
}

node! {
    #[tests(MyNodeSmokeTest)]
    fn my_node(x: u32) -> u32 { x }
}

fn main() {}
```

Notes:

- `#[tests(...)]` expects a list of paths (e.g. `#[tests(MyTestMarker, other::Marker)]`).
- Marker types are `pub use`’d only when `feature="export"` is enabled in the consumer crate (test macros gate marker exports behind `#[cfg(feature = "export")]`).
- Graphs implement `GraphUiTests` when `dto` or `export` is enabled (see `crates/macro/src/graph_macro/expand/metadata.rs`).

---

## Running tests

### From CLI

- `cargo test` runs the original Rust tests.

### From Graphium UI

Graphium UI (`crates/ui`) collects tests from configured graphs via `GraphUiTests::graphium_ui_tests()`.

The UI can then:

- List tests
- Render parameter inputs (derived by the test macro)
- Execute a test and display results (success / failure / panic message)

---

## Current limitations

- UI test runner does not support `async fn` tests yet (the macros mark them unsupported for UI).
- UI parameter typing is intentionally simple (text/number/bool).
