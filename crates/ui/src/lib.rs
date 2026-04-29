pub mod config;
pub mod error;
mod http;
mod logs;
mod mermaid;
mod metrics;
pub mod pages;
pub mod server;
pub mod state;
mod traces;
pub mod util;

pub use crate::config::GraphiumUiConfig;
pub use crate::error::UiError;
pub use crate::server::serve;
pub use crate::state::graph::{ConfiguredGraph, UiGraph, graph};
pub use crate::state::playground::Playground;

/// Convenience macro to build a `Vec<UiGraph>` from a list of graph *provider types*.
///
/// This macro is intended for configuration-style call sites where you want to list the graphs
/// your UI should expose, without writing repetitive `UiGraph::from_provider::<T>()`
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
///     $crate::UiGraph::from_provider::<a::A>(),
///     $crate::UiGraph::from_provider::<b::B>(),
/// ]
/// ```
///
/// # Notes
///
/// - `$crate::...` is used for macro hygiene so the path resolves to *this* crate's
///   `UiGraph` even when `graphs!` is invoked from another crate.
/// - The provider types passed to the macro must be valid type arguments for
///   `UiGraph::from_provider::<T>()` (i.e., satisfy whatever trait bounds that
///   constructor requires).
#[macro_export]
macro_rules! graphs {
    ($($graph:path),+ $(,)?) => {{
        vec![
            $(
                $crate::state::graph::UiGraph::from_provider::<$graph>()
            ),+
        ]
    }};
}
