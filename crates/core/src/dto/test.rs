use std::collections::HashMap;

use super::slugify;

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

#[derive(Clone)]
pub struct TestRun {
    pub dto: TestDto,
    pub schema: TestSchema,
    pub default_values: HashMap<String, String>,
    pub run: fn(&HashMap<String, String>) -> Result<(), String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestSchema {
    pub params: Vec<TestParam>,
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
