// Core helpers shared by generated graphs.
//
// `graph!` emits fully inlined orchestration code.

pub mod metrics;
pub mod test_registry;
pub mod visualizer;

pub use inventory;
pub use visualizer::{GraphCase, GraphDef, GraphDefProvider, GraphStep, Visualizer};

pub trait Artifact: Clone + 'static {}

impl<T> Artifact for T where T: Clone + 'static {}

/// Trait implemented by macro-generated graph configuration types.
///
/// The graph object describes and executes the plan.
pub trait Graph<Ctx> {
    /// Runs the graph with mutable context.
    fn run(ctx: &mut Ctx);
}

/// Clones or copies an artifact when one hop needs to fan out to more than one
/// immediate consumer.
pub fn clone_artifact<T: Artifact>(value: &T) -> T {
    value.clone()
}
