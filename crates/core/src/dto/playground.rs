use super::io::IoParamDto;


#[derive(Clone, Copy, Debug)]
pub struct PlaygroundParam {
    pub name: &'static str,
    pub ty: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct PlaygroundSchema {
    pub inputs: &'static [PlaygroundParam],
    pub outputs: &'static [PlaygroundParam],
    pub context: &'static str,
}

/// Optional UI integration for executing a graph from a web form.
///
/// This is primarily intended for local/dev tooling (Graphium UI).
pub trait GraphPlayground {
    /// Whether this graph can be executed by the generic UI playground runner.
    const PLAYGROUND_SUPPORTED: bool;

    fn playground_schema() -> PlaygroundSchema;

    fn playground_run(form: &std::collections::HashMap<String, String>) -> Result<String, String>;
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

