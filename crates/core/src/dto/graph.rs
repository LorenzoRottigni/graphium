use super::io::IoParamDto;

use super::{ctx::CtxAccessDto, node::NodeDto, playground::PlaygroundDto, test::TestDto};

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

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphCaseDto {
    pub label: String,
    pub steps: Vec<GraphStepDto>,
}
