//! Internal representation (IR) shared by parsing and code generation.
//!
//! This crate implements a few procedural macros (`graph!`, `node!`, plus test
//! helpers). While their public surface is expressed as token trees, the
//! implementations work best with a small typed model.
//!
//! Parsing converts macro input into these IR structures (e.g. `GraphInput`,
//! `NodeExpr`, `NodeDef`). The various expanders then walk the IR to generate
//! Rust code in a deterministic, easy-to-test way.
//!
//! Historically this module was called `shared`; it is now named `ir` to make
//! the "typed internal model" role explicit.

use quote::format_ident;
use std::collections::{BTreeMap, BTreeSet};
use syn::{Expr, Ident, Lifetime, Path, Type};

#[derive(Clone)]
pub struct NodeDef {
    pub fn_name: Ident,
    pub struct_name: Ident,
    pub ctx_type: Option<Type>,
    pub ctx_mut: bool,
    pub inputs: Vec<(Ident, Type)>,
    pub param_kinds: Vec<ParamKind>,
    pub return_ty: Option<Type>,
    pub metrics: MetricsSpec,
    pub return_is_result: bool,
    pub docs: Option<String>,
    pub tags: Vec<String>,
    pub deprecated: bool,
    pub deprecated_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParamKind {
    Ctx,
    Input(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BorrowSpec {
    pub lifetime: Option<Lifetime>,
    pub mutable: bool,
}

impl BorrowSpec {
    pub fn shared(lifetime: Option<Lifetime>) -> Self {
        Self {
            lifetime,
            mutable: false,
        }
    }

    pub fn mutable(lifetime: Option<Lifetime>) -> Self {
        Self {
            lifetime,
            mutable: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtifactInputKind {
    Owned,
    Borrowed(BorrowSpec),
    Taken(BorrowSpec),
}

#[derive(Clone)]
pub struct NodeCall {
    pub path: Path,
    pub explicit_inputs: bool,
    pub inputs: Vec<Ident>,
    pub input_kinds: Vec<ArtifactInputKind>,
    pub outputs: Vec<Ident>,
    pub output_borrows: Vec<Option<BorrowSpec>>,
}

#[derive(Clone)]
pub enum NodeExpr {
    Single(NodeCall),
    Sequence(Vec<NodeExpr>),
    Parallel(Vec<NodeExpr>),
    Route(RouteExpr),
    While(WhileExpr),
    Loop(LoopExpr),
    Break,
}

#[derive(Clone)]
pub struct RouteExpr {
    pub on: Expr,
    pub routes: Vec<(Expr, NodeExpr)>,
    pub outputs: Vec<Ident>,
    pub output_borrows: Vec<Option<BorrowSpec>>,
    pub is_if_chain: bool,
}

#[derive(Clone)]
pub struct WhileExpr {
    pub condition: Expr,
    pub body: Box<NodeExpr>,
    pub outputs: Vec<Ident>,
    pub output_borrows: Vec<Option<BorrowSpec>>,
}

#[derive(Clone)]
pub struct LoopExpr {
    pub body: Box<NodeExpr>,
    pub outputs: Vec<Ident>,
    pub output_borrows: Vec<Option<BorrowSpec>>,
}

pub struct GraphInput {
    pub attrs: Vec<syn::Attribute>,
    pub name: Ident,
    pub context: Path,
    pub lifetimes: Vec<Lifetime>,
    pub inputs: Vec<(Ident, Type)>,
    pub outputs: Vec<(Ident, Type)>,
    pub nodes: NodeExpr,
    pub async_enabled: bool,
    pub metrics: MetricsSpec,
    pub tests: Vec<Path>,
    pub tags: Vec<String>,
    pub deprecated: bool,
    pub deprecated_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MetricsSpec {
    pub performance: bool,
    pub errors: bool,
    pub count: bool,
    pub caller: bool,
    pub success_rate: bool,
    pub fail_rate: bool,
}

impl MetricsSpec {
    pub fn enabled(&self) -> bool {
        self.performance
            || self.errors
            || self.count
            || self.caller
            || self.success_rate
            || self.fail_rate
    }

    pub fn track_panics_sync(&self) -> bool {
        self.errors || self.success_rate || self.fail_rate
    }
}

pub fn parse_metric_name(value: &str) -> Option<fn(&mut MetricsSpec)> {
    match value {
        "performance" | "latency" => Some(|spec| spec.performance = true),
        "errors" | "error_rate" => Some(|spec| spec.errors = true),
        "count" => Some(|spec| spec.count = true),
        "caller" => Some(|spec| spec.caller = true),
        "success_rate" => Some(|spec| spec.success_rate = true),
        "fail_rate" => Some(|spec| spec.fail_rate = true),
        _ => None,
    }
}

pub fn doc_string_from_attrs(attrs: &[syn::Attribute]) -> Option<String> {
    let mut lines = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let syn::Meta::NameValue(name_value) = &attr.meta else {
            continue;
        };
        let syn::Expr::Lit(expr_lit) = &name_value.value else {
            continue;
        };
        let syn::Lit::Str(lit_str) = &expr_lit.lit else {
            continue;
        };
        let line = lit_str.value();
        lines.push(line.trim_start_matches(' ').to_string());
    }

    if lines.is_empty() {
        None
    } else {
        let joined = lines.join("\n");
        let trimmed = joined.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }
}

// `UsageMap` is compile-time bookkeeping only.
// For each artifact name, it stores how many consumers still need it inside
// the hop currently being generated.
pub type UsageMap = BTreeMap<String, usize>;

// A hop payload is the short-lived set of artifact variables that move from one
// `>>` boundary to the next. The values are generated local variable names.
#[derive(Clone, Default)]
pub struct Payload {
    pub owned: BTreeMap<String, Ident>,
    pub borrowed: BTreeSet<String>,
}

impl Payload {
    pub fn new() -> Self {
        Self {
            owned: BTreeMap::new(),
            borrowed: BTreeSet::new(),
        }
    }

    pub fn insert_owned(&mut self, name: String, ident: Ident) {
        self.owned.insert(name, ident);
    }

    pub fn insert_borrowed(&mut self, name: String) {
        self.borrowed.insert(name);
    }

    pub fn get_owned(&self, name: &str) -> Option<&Ident> {
        self.owned.get(name)
    }

    pub fn has_borrowed(&self, name: &str) -> bool {
        self.borrowed.contains(name)
    }

    pub fn is_empty(&self) -> bool {
        self.owned.is_empty() && self.borrowed.is_empty()
    }
}

// `ExprShape` is a lightweight summary of a subgraph.
// It tells the parent expression:
// - which artifacts must be present at entry
// - which artifact names can come out at exit
#[derive(Clone)]
pub struct ExprShape {
    pub entry_usage: UsageMap,
    pub entry_borrowed: BTreeSet<String>,
    pub exit_outputs: Vec<String>,
    pub exit_borrowed: BTreeSet<String>,
}

// Result of generating one graph expression.
// `tokens` is the emitted Rust code, `outputs` is the payload owned by the
// expression when that code finishes running.
pub struct GeneratedExpr {
    pub tokens: proc_macro2::TokenStream,
    pub outputs: Payload,
}

/// Converts a snake_case function name into the PascalCase node wrapper name
/// generated by `node!`.
pub fn pascal_case(ident: &Ident) -> String {
    ident
        .to_string()
        .split('_')
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<String>()
}

// Macro-generated locals need stable, collision-free names.
// We thread a counter through codegen and attach both a purpose prefix and the
// artifact name to keep debug output somewhat readable.
/// Builds a unique local identifier used inside generated graph code.
pub fn fresh_ident(counter: &mut usize, prefix: &str, name: &str) -> Ident {
    let ident = format_ident!("__graphium_{}_{}_{}", prefix, *counter, name);
    *counter += 1;
    ident
}

/// Builds a stable identifier used for graph-scoped borrowed artifact storage.
///
/// These locals are declared once in the generated `Graph::run{_async}` body
/// and are shared across all hops/branches within that graph execution.
pub fn borrowed_slot_ident(name: &str) -> Ident {
    format_ident!("__graphium_borrowed_{}", name)
}

// `FooGraph::run()` is treated specially inside the DSL: it executes another
// graph directly instead of behaving like a node call with hop-managed artifacts.
/// Returns `true` when a path refers to another graph's `run` function instead
/// of a normal node wrapper.
pub fn is_graph_run_path(path: &Path) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == "run")
}
