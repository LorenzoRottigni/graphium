pub use graphium_ui_next::*;
pub use graphium_ui_next::{config, server};

/// Convenience macro to build a `Vec<ConfiguredGraph>` from a list of graph *provider types*.
///
/// This macro is intended for configuration-style call sites where you want to list the graphs
/// your UI should expose, without writing repetitive `ConfiguredGraph::from_provider::<T>()`
/// calls.
///
/// # Syntax
///
/// ```ignore
/// graphs!(TypeA, TypeB, path::to::TypeC,);
/// ```
///
/// - Accepts **one or more** arguments.
/// - Each argument must be a Rust `path` (a type name or fully-qualified type path).
/// - Allows an **optional trailing comma**.
///
/// # Expansion
///
/// The invocation:
///
/// ```ignore
/// graphs!(a::A, b::B)
/// ```
///
/// expands to code equivalent to:
///
/// ```ignore
/// vec![
///     $crate::ConfiguredGraph::from_provider::<a::A>(),
///     $crate::ConfiguredGraph::from_provider::<b::B>(),
/// ]
/// ```
///
/// # Notes
///
/// - `$crate::...` is used for macro hygiene so the path resolves to *this* crate's
///   `ConfiguredGraph` even when `graphs!` is invoked from another crate.
/// - The provider types passed to the macro must be valid type arguments for
///   `ConfiguredGraph::from_provider::<T>()` (i.e., satisfy whatever trait bounds that
///   constructor requires).
#[macro_export]
macro_rules! graphs {
    ($($graph:path),+ $(,)?) => {{
        vec![
            $(
                $crate::ConfiguredGraph::from_provider::<$graph>()
            ),+
        ]
    }};
}
