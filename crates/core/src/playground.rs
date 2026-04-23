#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CtxAccess {
    None,
    Ref,
    Mut,
}

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

