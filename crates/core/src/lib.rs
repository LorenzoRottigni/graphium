// Core helpers shared by generated graphs.
//
// `graph!` emits fully inlined orchestration code.

pub mod dto;
pub mod telemetry;

#[cfg(feature = "export")]
pub use serde;

pub use dto::ctx::CtxAccess;
pub use dto::playground::{GraphPlayground, PlaygroundParam, PlaygroundSchema};
pub use telemetry::GraphiumTelemetry;

/// Backwards-compatible module path; prefer `graphium::dto`.
pub mod export {
    pub use super::dto::*;
}

pub trait Artifact: Clone + 'static {}

impl<T> Artifact for T where T: Clone + 'static {}

#[cfg(feature = "macros")]
pub use graphium_macro::{graph, graph_test, node, node_test};

/// Trait implemented by macro-generated graph configuration types.
///
/// The graph object describes and executes the plan.
pub trait Graph<Ctx> {
    /// Runs the graph with mutable context.
    fn run(ctx: &mut Ctx);
}

/// Trait implemented by macro-generated node wrapper types so they can be
/// passed as typed "handles" to other nodes.
///
/// `Inputs` is represented as a tuple, e.g. `()`, `(A,)`, `(A, B)`.
pub trait NodeHandle<Ctx, Inputs, Output> {
    fn run(&self, ctx: &Ctx, inputs: Inputs) -> Output;
}

/// Variant for nodes that require mutable context access.
pub trait NodeHandleMut<Ctx, Inputs, Output> {
    fn run(&self, ctx: &mut Ctx, inputs: Inputs) -> Output;
}

/// Trait implemented by macro-generated graphs so they can be passed as typed
/// "handles" to other nodes.
///
/// `Inputs` is represented as a tuple, e.g. `()`, `(A,)`, `(A, B)`.
pub trait GraphHandle<Ctx, Inputs, Output> {
    fn run(&self, ctx: &mut Ctx, inputs: Inputs) -> Output;
}

/// Trait implemented by macro-generated graphs to expose UI/admin test runners
/// without any runtime registry/discovery.
pub trait GraphUiTests {
    fn graphium_ui_tests() -> Vec<dto::test::TestRun>;
}

/// Clones or copies an artifact when one hop needs to fan out to more than one
/// immediate consumer.
pub fn clone_artifact<T: Artifact>(value: &T) -> T {
    value.clone()
}

#[derive(Default)]
pub struct Context {}
