use std::collections::HashMap;

#[derive(Clone)]
pub(crate) struct UiTest {
    pub(crate) dto: graphium::export::TestDto,
    pub(crate) schema: graphium::export::TestSchema,
    pub(crate) default_values: HashMap<String, String>,
    pub(crate) run: fn(&HashMap<String, String>) -> Result<(), String>,
    pub(crate) graph_name: String,
    pub(crate) graph_id: String,
}

impl UiTest {
    pub(crate) fn kind_label(&self) -> &'static str {
        match self.dto.kind {
            graphium::export::TestKindDto::Node => "Node",
            graphium::export::TestKindDto::Graph => "Graph",
        }
    }

    pub(crate) fn run(&self) -> TestExecution {
        self.run_with(&self.default_values)
    }

    pub(crate) fn run_with(&self, values: &HashMap<String, String>) -> TestExecution {
        match (self.run)(values) {
            Ok(()) => TestExecution {
                passed: true,
                message: "ok".to_string(),
            },
            Err(err) => TestExecution {
                passed: false,
                message: err,
            },
        }
    }
}

#[derive(Clone)]
pub(crate) struct TestExecution {
    pub(crate) passed: bool,
    pub(crate) message: String,
}
