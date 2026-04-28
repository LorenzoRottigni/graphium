use super::{test::TestDto, ctx::CtxAccessDto, playground::PlaygroundSchemaDto};

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
