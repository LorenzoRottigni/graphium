//! Code generation for the `graph!` procedural macro.
//!
//! The graph expander is split by concern so the hop-based orchestration rules
//! are easier to navigate and test.
//!
//! **Where to look**
//! - `parse`: implements the `syn::Parse` logic for the graph DSL and produces
//!   `crate::ir::GraphInput` / `crate::ir::NodeExpr`.
//! - `analysis`: computes static "shape" information (required inputs / possible
//!   outputs) used for validation and DTO rendering.
//! - `expr`: expands `NodeExpr` into executable Rust code (the bulk of codegen).
//! - `execution`: wraps expression expansion into the final `run` / `run_async`
//!   method bodies.
//! - `expand`: the macro entry point that ties everything together and emits
//!   the final graph type + feature-gated helpers.

mod analysis;
mod execution;
mod expand;
mod expr;
mod parse;

pub use expand::expand;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_expand_entrypoint() {
        let _entry: fn(proc_macro::TokenStream) -> proc_macro::TokenStream = expand;
    }
}
