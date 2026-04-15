use proc_macro::TokenStream;

mod graph;
mod node;
mod node_test;
mod parser;
mod shared;

/// Expands a `node! { ... }` item into a wrapper type plus a uniform
/// `__graphium_run` entry point used by generated graphs.
#[proc_macro]
pub fn node(input: TokenStream) -> TokenStream {
    node::expand(input)
}

/// Expands a `graph! { ... }` definition into hop-based orchestration code
/// that wires artifacts between adjacent graph steps.
#[proc_macro]
pub fn graph(input: TokenStream) -> TokenStream {
    graph::expand(input)
}

/// Expands grouped node-scoped tests while preserving idiomatic Rust test
/// definitions (`#[test]`, `#[tokio::test]`, etc.).
#[proc_macro]
pub fn node_test(input: TokenStream) -> TokenStream {
    node_test::expand(input)
}
