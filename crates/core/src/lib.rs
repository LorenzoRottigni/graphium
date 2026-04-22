// Core helpers shared by generated graphs.
//
// `graph!` emits fully inlined orchestration code.

pub mod export;
pub mod metrics;
pub mod visualizer;

#[cfg(feature = "serialize")]
pub use serde;

pub use visualizer::{
    CtxAccess, GraphCase, GraphDef, GraphDefProvider, GraphPlayground, GraphStep, PlaygroundParam,
    PlaygroundSchema, Visualizer,
};

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

/// Trait implemented by macro-generated graphs to expose UI/admin test runners
/// without any runtime registry/discovery.
pub trait GraphUiTests {
    fn graphium_ui_tests() -> Vec<export::TestRun>;
}

/// Clones or copies an artifact when one hop needs to fan out to more than one
/// immediate consumer.
pub fn clone_artifact<T: Artifact>(value: &T) -> T {
    value.clone()
}

#[derive(Default)]
pub struct Context {}
