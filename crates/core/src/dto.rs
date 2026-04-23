//! DTOs for Graphium graphs and nodes.
//!
//! These types are intended for tooling (e.g. graphium-ui) and are designed to
//! be stable, serde-serializable data structures.

use crate::{CtxAccess, PlaygroundSchema};
use std::collections::HashMap;

pub const EXPORT_SCHEMA_VERSION: u32 = 2;

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphiumBundleDto {
    pub schema_version: u32,
    pub graphs: Vec<GraphDto>,
    pub nodes: Vec<NodeDto>,
}

impl GraphiumBundleDto {
    pub fn new() -> Self {
        Self {
            schema_version: EXPORT_SCHEMA_VERSION,
            graphs: Vec::new(),
            nodes: Vec::new(),
        }
    }

    pub fn from_graph_roots(roots: &[GraphDto]) -> Self {
        let mut bundle = Self::new();
        for graph in roots {
            bundle.insert_graph_recursive(graph);
        }
        bundle.dedupe();
        bundle
    }

    pub fn dedupe(&mut self) {
        use std::collections::HashSet;

        let mut seen_graphs: HashSet<String> = HashSet::new();
        self.graphs.retain(|g| seen_graphs.insert(g.id.clone()));

        let mut seen_nodes: HashSet<String> = HashSet::new();
        self.nodes.retain(|n| seen_nodes.insert(n.id.clone()));
    }

    fn insert_graph_recursive(&mut self, graph: &GraphDto) {
        self.graphs.push(graph.clone());
        self.nodes.extend(graph.nodes.iter().cloned());
        for sub in &graph.subgraphs {
            self.insert_graph_recursive(sub);
        }
    }
}

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphDto {
    pub id: String,
    pub name: String,
    pub docs: Option<String>,
    pub tags: Vec<String>,
    pub deprecated: bool,
    pub deprecated_reason: Option<String>,
    pub schema: Option<GraphSchemaDto>,
    pub flow: GraphFlowDto,
    /// Raw schema definition text (typically the `graph! { ... }` tokens).
    pub raw_schema: Option<String>,
    /// Tests explicitly attached to this graph (UI/admin build only).
    pub tests: Vec<TestDto>,
    /// Nodes referenced directly by this graph.
    pub nodes: Vec<NodeDto>,
    /// Nested graphs referenced directly by this graph.
    pub subgraphs: Vec<GraphDto>,
    pub playground: Option<PlaygroundDto>,
}

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphSchemaDto {
    pub context: String,
    pub inputs: Vec<IoParamDto>,
    pub outputs: Vec<IoParamDto>,
    pub metrics: Vec<String>,
}

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphFlowDto {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub steps: Vec<GraphStepDto>,
}

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IoParamDto {
    pub name: String,
    pub ty: String,
}

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaygroundDto {
    pub supported: bool,
    pub schema: PlaygroundSchemaDto,
}

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaygroundSchemaDto {
    pub inputs: Vec<IoParamDto>,
    pub outputs: Vec<IoParamDto>,
    pub context: String,
}

impl PlaygroundSchemaDto {
    pub fn from_schema(schema: &PlaygroundSchema) -> Self {
        Self {
            inputs: schema
                .inputs
                .iter()
                .map(|p| IoParamDto {
                    name: p.name.to_string(),
                    ty: p.ty.to_string(),
                })
                .collect(),
            outputs: schema
                .outputs
                .iter()
                .map(|p| IoParamDto {
                    name: p.name.to_string(),
                    ty: p.ty.to_string(),
                })
                .collect(),
            context: schema.context.to_string(),
        }
    }
}

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NodeDto {
    pub id: String,
    pub target: String,
    pub label: String,
    pub docs: Option<String>,
    pub tags: Vec<String>,
    pub deprecated: bool,
    pub deprecated_reason: Option<String>,
    /// Raw node definition text (typically the `node! { ... }` tokens).
    pub raw_schema: Option<String>,
    /// Tests explicitly attached to this node (UI/admin build only).
    pub tests: Vec<TestDto>,
    pub ctx_access: CtxAccessDto,
    pub metrics_graph: String,
    pub metrics_node: String,
    pub playground_supported: bool,
    pub playground_schema: PlaygroundSchemaDto,
}

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TestKindDto {
    #[default]
    Node,
    Graph,
}

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestDto {
    pub id: String,
    pub name: String,
    pub kind: TestKindDto,
    pub target: String,
    pub target_id: String,
}

impl TestDto {
    pub fn new(kind: TestKindDto, name: &'static str, target: &'static str) -> Self {
        let kind_prefix = match kind {
            TestKindDto::Node => "node",
            TestKindDto::Graph => "graph",
        };
        let target_last = target.rsplit("::").next().unwrap_or(target);
        let id = format!("{kind_prefix}-{}-{}", slugify(target_last), slugify(name));
        Self {
            id,
            name: name.to_string(),
            kind,
            target: target.to_string(),
            target_id: slugify(target_last),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestParamKind {
    Text,
    Number,
    Bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestParam {
    pub name: String,
    pub kind: TestParamKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestSchema {
    pub params: Vec<TestParam>,
}

#[derive(Clone)]
pub struct TestRun {
    pub dto: TestDto,
    pub schema: TestSchema,
    pub default_values: HashMap<String, String>,
    pub run: fn(&HashMap<String, String>) -> Result<(), String>,
}

pub fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(msg) = payload.downcast_ref::<&'static str>() {
        return (*msg).to_string();
    }
    if let Some(msg) = payload.downcast_ref::<String>() {
        return msg.clone();
    }
    "panic while running test".to_string()
}

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CtxAccessDto {
    #[default]
    None,
    Ref,
    Mut,
}

impl From<CtxAccess> for CtxAccessDto {
    fn from(value: CtxAccess) -> Self {
        match value {
            CtxAccess::None => Self::None,
            CtxAccess::Ref => Self::Ref,
            CtxAccess::Mut => Self::Mut,
        }
    }
}

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphCaseDto {
    pub label: String,
    pub steps: Vec<GraphStepDto>,
}

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphRefDto {
    pub id: String,
    pub name: String,
}

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphStepDto {
    Node {
        name: String,
        ctx: CtxAccessDto,
        inputs: Vec<String>,
        outputs: Vec<String>,
    },
    Nested {
        graph: GraphRefDto,
        ctx: CtxAccessDto,
        inputs: Vec<String>,
        outputs: Vec<String>,
    },
    Parallel {
        branches: Vec<Vec<GraphStepDto>>,
        inputs: Vec<String>,
        outputs: Vec<String>,
    },
    Route {
        on: String,
        cases: Vec<GraphCaseDto>,
        inputs: Vec<String>,
        outputs: Vec<String>,
    },
    While {
        condition: String,
        body: Vec<GraphStepDto>,
        inputs: Vec<String>,
        outputs: Vec<String>,
    },
    Loop {
        body: Vec<GraphStepDto>,
        inputs: Vec<String>,
        outputs: Vec<String>,
    },
    Break,
}

impl Default for GraphStepDto {
    fn default() -> Self {
        Self::Break
    }
}

pub fn slugify(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut prev_dash = false;
    for ch in value.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}
