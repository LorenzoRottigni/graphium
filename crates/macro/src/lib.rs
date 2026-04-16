use proc_macro::TokenStream;

mod graph_macro;
mod node_macro;
mod parser;
mod shared;
mod test_macro;

/// Expands a `node! { ... }` item into a wrapper type plus a uniform
/// `__graphium_run` entry point used by generated graphs.
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
