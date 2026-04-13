// Core runtime helpers shared by generated graphs.
//
// `graph!` emits fully inlined orchestration code.
// `graph_runtime!` emits a runtime plan that is interpreted by
// `RuntimeController` below.

use std::any::Any;
use std::collections::BTreeMap;

pub trait Artifact: Clone + 'static {}

impl<T> Artifact for T where T: Clone + 'static {}

/// Trait implemented by macro-generated graph configuration types.
///
/// The graph object describes and executes the plan.
pub trait Graph<Ctx> {
    /// Runs the graph with mutable context.
    fn run(ctx: &mut Ctx);
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

/// Type-erased artifact storage used by runtime graph execution.
pub struct RuntimeArtifacts {
    values: BTreeMap<&'static str, RuntimeValue>,
}

impl RuntimeArtifacts {
    pub fn new() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn contains(&self, name: &'static str) -> bool {
        self.values.contains_key(name)
    }

    pub fn keys(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.values.keys().copied()
    }

    pub fn insert<T: Artifact>(&mut self, name: &'static str, value: T) {
        self.values.insert(name, RuntimeValue::new(value));
    }

    pub fn take<T: 'static>(&mut self, name: &'static str) -> T {
        let boxed = self
            .values
            .remove(name)
            .unwrap_or_else(|| panic!("missing artifact `{name}`"));
        match boxed.value.downcast::<T>() {
            Ok(value) => *value,
            Err(_) => panic!("artifact `{name}` has unexpected type"),
        }
    }

    pub fn clone_value<T: Artifact>(&self, name: &'static str) -> T {
        let value = self
            .values
            .get(name)
            .unwrap_or_else(|| panic!("missing artifact `{name}`"));
        match value.value.downcast_ref::<T>() {
            Some(typed) => typed.clone(),
            None => panic!("artifact `{name}` has unexpected type"),
        }
    }

    fn take_dyn(&mut self, name: &'static str) -> RuntimeValue {
        self.values
            .remove(name)
            .unwrap_or_else(|| panic!("missing artifact `{name}`"))
    }

    fn insert_dyn(&mut self, name: &'static str, value: RuntimeValue) {
        self.values.insert(name, value);
    }

    fn clone_dyn(&self, name: &'static str) -> RuntimeValue {
        self.values
            .get(name)
            .unwrap_or_else(|| panic!("missing artifact `{name}`"))
            .clone()
    }

    fn take_required(&mut self, required: &[&'static str]) -> RuntimeArtifacts {
        let mut payload = RuntimeArtifacts::new();
        for artifact in required {
            payload.insert_dyn(artifact, self.take_dyn(artifact));
        }
        payload
    }
}

impl Default for RuntimeArtifacts {
    fn default() -> Self {
        Self::new()
    }
}

struct RuntimeValue {
    value: Box<dyn Any>,
    clone_fn: fn(&dyn Any) -> Box<dyn Any>,
}

impl RuntimeValue {
    fn new<T: Artifact>(value: T) -> Self {
        fn clone_impl<T: Artifact>(value: &dyn Any) -> Box<dyn Any> {
            let typed = value
                .downcast_ref::<T>()
                .unwrap_or_else(|| panic!("artifact has unexpected runtime type"));
            Box::new(typed.clone())
        }

        Self {
            value: Box::new(value),
            clone_fn: clone_impl::<T>,
        }
    }
}

impl Clone for RuntimeValue {
    fn clone(&self) -> Self {
        Self {
            value: (self.clone_fn)(self.value.as_ref()),
            clone_fn: self.clone_fn,
        }
    }
}

pub type RuntimeNodeRunner<Ctx> = fn(&mut Ctx, &mut RuntimeArtifacts) -> RuntimeArtifacts;
pub type RuntimeRouteSelector<Ctx> = fn(&mut Ctx) -> usize;

pub struct RuntimeNodeExec<Ctx> {
    pub id: usize,
    pub name: &'static str,
    pub run: RuntimeNodeRunner<Ctx>,
    pub entry: Vec<&'static str>,
    pub exits: Vec<&'static str>,
}

pub struct RuntimeSequenceExec<Ctx> {
    pub steps: Vec<RuntimeExpr<Ctx>>,
    pub entry: Vec<&'static str>,
    pub exits: Vec<&'static str>,
}

pub struct RuntimeParallelExec<Ctx> {
    pub branches: Vec<RuntimeExpr<Ctx>>,
    pub entry: Vec<&'static str>,
    pub exits: Vec<&'static str>,
}

pub struct RuntimeRouteExec<Ctx> {
    pub select: RuntimeRouteSelector<Ctx>,
    pub branches: Vec<RuntimeExpr<Ctx>>,
    pub entry: Vec<&'static str>,
    pub exits: Vec<&'static str>,
}

pub enum RuntimeExpr<Ctx> {
    Node(RuntimeNodeExec<Ctx>),
    Sequence(RuntimeSequenceExec<Ctx>),
    Parallel(RuntimeParallelExec<Ctx>),
    Route(RuntimeRouteExec<Ctx>),
}

impl<Ctx> RuntimeExpr<Ctx> {
    fn entry_artifacts(&self) -> &[&'static str] {
        match self {
            RuntimeExpr::Node(node) => &node.entry,
            RuntimeExpr::Sequence(sequence) => &sequence.entry,
            RuntimeExpr::Parallel(parallel) => &parallel.entry,
            RuntimeExpr::Route(route) => &route.entry,
        }
    }
}

/// Stateful graph definition that an external runtime can inspect and drive.
pub struct RuntimeGraph<Ctx> {
    pub name: &'static str,
    pub nodes: &'static [RuntimeNode],
    pub edges: &'static [RuntimeEdge],
    pub root: RuntimeExpr<Ctx>,
    pub inputs: Vec<&'static str>,
    pub outputs: Vec<&'static str>,
    pub state: RuntimeGraphState,
    _ctx: ::core::marker::PhantomData<Ctx>,
}

impl<Ctx> RuntimeGraph<Ctx> {
    /// Creates a new runtime graph definition.
    pub fn new(
        name: &'static str,
        nodes: &'static [RuntimeNode],
        edges: &'static [RuntimeEdge],
        root: RuntimeExpr<Ctx>,
        inputs: Vec<&'static str>,
        outputs: Vec<&'static str>,
    ) -> Self {
        Self {
            name,
            nodes,
            edges,
            root,
            inputs,
            outputs,
            state: RuntimeGraphState { runs: 0 },
            _ctx: ::core::marker::PhantomData,
        }
    }
}

/// Runtime interpreter for a `RuntimeGraph` plan generated by `graph_runtime!`.
#[derive(Default)]
pub struct RuntimeController;

impl RuntimeController {
    pub fn execute<Ctx>(
        &self,
        graph: &mut RuntimeGraph<Ctx>,
        ctx: &mut Ctx,
        mut inputs: RuntimeArtifacts,
    ) -> RuntimeArtifacts {
        let _ = self;
        for required in &graph.inputs {
            if !inputs.contains(required) {
                panic!("missing graph input `{required}`");
            }
        }

        let mut outputs = execute_expr(&graph.root, ctx, &mut inputs);

        graph.state.runs += 1;

        if graph.outputs.is_empty() {
            RuntimeArtifacts::new()
        } else {
            outputs.take_required(&graph.outputs)
        }
    }
}

fn execute_expr<Ctx>(
    expr: &RuntimeExpr<Ctx>,
    ctx: &mut Ctx,
    incoming: &mut RuntimeArtifacts,
) -> RuntimeArtifacts {
    match expr {
        RuntimeExpr::Node(node) => (node.run)(ctx, incoming),
        RuntimeExpr::Sequence(sequence) => execute_sequence(sequence, ctx, incoming),
        RuntimeExpr::Parallel(parallel) => execute_parallel(parallel, ctx, incoming),
        RuntimeExpr::Route(route) => execute_route(route, ctx, incoming),
    }
}

fn execute_sequence<Ctx>(
    sequence: &RuntimeSequenceExec<Ctx>,
    ctx: &mut Ctx,
    incoming: &mut RuntimeArtifacts,
) -> RuntimeArtifacts {
    let mut iter = sequence.steps.iter();
    let first = iter
        .next()
        .unwrap_or_else(|| panic!("sequence must contain at least one step"));
    let mut current = execute_expr(first, ctx, incoming);

    for next in iter {
        let mut payload = current.take_required(next.entry_artifacts());
        current = execute_expr(next, ctx, &mut payload);
    }

    current
}

fn execute_parallel<Ctx>(
    parallel: &RuntimeParallelExec<Ctx>,
    ctx: &mut Ctx,
    incoming: &mut RuntimeArtifacts,
) -> RuntimeArtifacts {
    let mut remaining = BTreeMap::new();
    for branch in &parallel.branches {
        for artifact in branch.entry_artifacts() {
            *remaining.entry(*artifact).or_insert(0usize) += 1;
        }
    }

    let mut merged = RuntimeArtifacts::new();
    for branch in &parallel.branches {
        let mut payload = RuntimeArtifacts::new();
        for artifact in branch.entry_artifacts() {
            let rem = remaining
                .get_mut(artifact)
                .unwrap_or_else(|| panic!("missing usage count for `{artifact}`"));

            if *rem == 1 {
                payload.insert_dyn(artifact, incoming.take_dyn(artifact));
            } else {
                payload.insert_dyn(artifact, incoming.clone_dyn(artifact));
            }
            *rem -= 1;
        }

        let branch_outputs = execute_expr(branch, ctx, &mut payload);
        for (name, value) in branch_outputs.values {
            if merged.values.contains_key(name) {
                panic!("parallel step produces duplicate artifact `{name}`");
            }
            merged.insert_dyn(name, value);
        }
    }

    merged
}

fn execute_route<Ctx>(
    route: &RuntimeRouteExec<Ctx>,
    ctx: &mut Ctx,
    incoming: &mut RuntimeArtifacts,
) -> RuntimeArtifacts {
    let selected = (route.select)(ctx);
    let branch = route
        .branches
        .get(selected)
        .unwrap_or_else(|| panic!("route selector returned invalid branch index {selected}"));

    let mut payload = incoming.take_required(branch.entry_artifacts());
    execute_expr(branch, ctx, &mut payload)
}

/// Clones or copies an artifact when one hop needs to fan out to more than one
/// immediate consumer.
pub fn clone_artifact<T: Artifact>(value: &T) -> T {
    value.clone()
}
