//! Code generation for the `node!` procedural macro.
//!
//! Node expansion is intentionally simple now. A node macro only validates the
//! user function and generates a thin wrapper exposing a uniform
//! `run` entry point. Artifact propagation is handled entirely by
//! `graph!`.

mod expand;
mod metrics;
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
