use proc_macro::TokenStream;

mod graph;
mod graph_runtime;
mod node;
mod parser;
mod shared;

/// Expands a `node! { ... }` item into a wrapper type plus a uniform
/// `__graphio_run` entry point used by generated graphs.
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

/// Expands a `graph_runtime! { ... }` definition into a runtime graph value
/// factory with node/edge metadata plus executable behavior.
#[proc_macro]
pub fn graph_runtime(input: TokenStream) -> TokenStream {
    graph_runtime::expand(input)
}
