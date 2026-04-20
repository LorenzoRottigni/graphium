use std::collections::HashMap;

#[derive(Clone)]
pub(crate) struct UiTest {
    pub(crate) dto: graphium::export::TestDto,
    pub(crate) schema: graphium::export::TestSchema,
    pub(crate) default_values: HashMap<String, String>,
    pub(crate) run: fn(&HashMap<String, String>) -> Result<(), String>,
}
