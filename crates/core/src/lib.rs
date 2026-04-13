// Core runtime helpers shared by generated graphs.
// The runtime surface is intentionally tiny: the macro emits mostly plain Rust
// and only relies on this trait/function pair to express "this value can be
// duplicated when a hop fans out to multiple consumers".

pub trait Artifact: Clone + 'static {}

impl<T> Artifact for T where T: Clone + 'static {}

/// Trait implemented by macro-generated graph configuration types.
///
/// The graph object describes and executes the plan.
pub trait Graph<Ctx> {
    /// Executes the graph with mutable context.
    fn execute(&self, ctx: &mut Ctx);
}

/// Runtime description of a node in a stateful graph definition.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeNode {
    pub id: usize,
    pub name: &'static str,
}

/// Runtime description of a directed edge between two runtime node IDs.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeEdge {
    pub from: usize,
    pub to: usize,
}

/// Mutable runtime state tracked for a runtime graph instance.
#[derive(Debug, Default, Clone, Copy)]
pub struct RuntimeGraphState {
    pub runs: usize,
}

/// Stateful graph definition that an external runtime can inspect
/// and execute.
pub struct RuntimeGraph<Ctx> {
    pub name: &'static str,
    pub nodes: &'static [RuntimeNode],
    pub edges: &'static [RuntimeEdge],
    pub state: RuntimeGraphState,
    executor: fn(&mut Ctx),
}

impl<Ctx> RuntimeGraph<Ctx> {
    /// Creates a new runtime graph definition.
    pub const fn new(
        name: &'static str,
        nodes: &'static [RuntimeNode],
        edges: &'static [RuntimeEdge],
        executor: fn(&mut Ctx),
    ) -> Self {
        Self {
            name,
            nodes,
            edges,
            state: RuntimeGraphState { runs: 0 },
            executor,
        }
    }

    /// Executes this runtime graph.
    pub fn execute(&self, ctx: &mut Ctx) {
        (self.executor)(ctx);
    }

    /// Executes this runtime graph and updates mutable runtime state.
    pub fn run(&mut self, ctx: &mut Ctx) {
        self.state.runs += 1;
        self.execute(ctx);
    }

    /// Returns how many times this runtime graph instance was run.
    pub fn runs(&self) -> usize {
        self.state.runs
    }
}

/// Clones or copies an artifact when one hop needs to fan out to more than one
/// immediate consumer.
pub fn clone_artifact<T: Artifact>(value: &T) -> T {
    value.clone()
}
