//! Code generation for `graph_test!` and `node_test!` procedural macros.
//!
//! These macros expand test suites and (when `feature="export"` is enabled in
//! the destination crate) generate marker types that can be referenced by
//! `graph!` / `node!` via `#[tests(...)]`.

mod graph;
mod node;
mod util;

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
