use proc_macro::TokenStream;

mod graph;
mod node;
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
