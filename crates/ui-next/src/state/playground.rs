use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct Playground {
    pub(crate) supported: bool,
    pub(crate) schema: graphium::PlaygroundSchema,
    pub(crate) run: fn(&HashMap<String, String>) -> Result<String, String>,
}
