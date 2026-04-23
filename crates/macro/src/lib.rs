//! Procedural macros for Graphium.
//!
//! This crate is deliberately organized around **macro entry points** rather
//! than around token-level manipulation:
//!
//! - `node! { ... }` parses a Rust function and emits a small wrapper type with
//!   a uniform `run` / `run_async` interface.
//! - `graph! { ... }` parses a small DSL and emits orchestration code that
//!   forwards artifacts hop-by-hop between nodes.
//! - `graph_test!` / `node_test!` are helper macros for grouping tests and
//!   exporting metadata for UI tooling.
//!
//! Internally the implementation follows a consistent pipeline:
//!
//! 1. **Parse** macro input into a small typed IR (`crate::ir`).
//! 2. **Analyze** the IR where needed (e.g. branch/loop contracts).
//! 3. **Expand** the IR into Rust code using `quote!` and `syn`.
//!
//! Keeping parsing and codegen separated makes the macros easier to maintain:
//! most logic operates on `crate::ir::*` instead of raw token streams.

use proc_macro::TokenStream;

mod graph_macro;
mod ir;
mod node_macro;
mod test_macro;

/// Expands a `node! { ... }` item into a wrapper type plus a uniform
/// `run` entry point used by generated graphs.
#[proc_macro]
pub fn node(input: TokenStream) -> TokenStream {
    node_macro::expand(input)
}

/// Expands a `graph! { ... }` definition into hop-based orchestration code
/// that wires artifacts between adjacent graph steps.
#[proc_macro]
pub fn graph(input: TokenStream) -> TokenStream {
    graph_macro::expand(input)
}

/// Expands grouped graph-scoped tests while preserving idiomatic Rust test
/// definitions (`#[test]`, `#[tokio::test]`, etc.).
#[proc_macro]
pub fn graph_test(input: TokenStream) -> TokenStream {
    test_macro::graph_expand(input)
}

/// Expands grouped node-scoped tests while preserving idiomatic Rust test
/// definitions (`#[test]`, `#[tokio::test]`, etc.).
#[proc_macro]
pub fn node_test(input: TokenStream) -> TokenStream {
    test_macro::node_expand(input)
}
