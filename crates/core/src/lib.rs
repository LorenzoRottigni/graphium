// Core runtime helpers shared by generated graphs.
// The runtime surface is intentionally tiny: the macro emits mostly plain Rust
// and only relies on this trait/function pair to express "this value can be
// duplicated when a hop fans out to multiple consumers".

pub trait Artifact: Clone + 'static {}

impl<T> Artifact for T where T: Clone + 'static {}

/// Trait implemented by macro-generated graph configuration types.
///
/// The graph object describes the execution plan while the controller owns the
/// runtime policy that drives that plan.
pub trait Graph<Ctx> {
    /// Executes the graph with the provided controller and mutable context.
    fn execute(&self, controller: &Controller, ctx: &mut Ctx);
}

/// Runtime entry point responsible for driving graph execution.
///
/// Today the controller mainly delegates into macro-generated code, but moving
/// execution through this type gives the crate a natural home for future
/// concerns such as tracing, metrics, cancellation, retries, or loop guards.
#[derive(Debug, Default, Clone, Copy)]
pub struct Controller;

impl Controller {
    /// Creates a new controller with default runtime behavior.
    pub const fn new() -> Self {
        Self
    }

    /// Runs a configured graph with this controller.
    pub fn run<G, Ctx>(&self, graph: &G, ctx: &mut Ctx)
    where
        G: Graph<Ctx>,
    {
        graph.execute(self, ctx);
    }
}

/// Clones or copies an artifact when one hop needs to fan out to more than one
/// immediate consumer.
pub fn clone_artifact<T: Artifact>(value: &T) -> T {
    value.clone()
}
