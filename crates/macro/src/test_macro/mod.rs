//! Code generation for `graph_test!` and `node_test!` procedural macros.
//!
//! These macros expand test suites with support for runtime-discoverable UI tests
//! through the `#[for_graph(...)]` and `#[for_node(...)]` attributes.

mod graph;
mod node;
mod registry;

pub use graph::expand as graph_expand;
pub use node::expand as node_expand;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_graph_expand_entrypoint() {
        let _entry: fn(proc_macro::TokenStream) -> proc_macro::TokenStream = graph_expand;
    }

    #[test]
    fn exports_node_expand_entrypoint() {
        let _entry: fn(proc_macro::TokenStream) -> proc_macro::TokenStream = node_expand;
    }
}
