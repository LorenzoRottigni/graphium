use super::playground::Playground;

#[derive(Clone)]
pub struct UiGraph {
    pub id: String,
    pub name: String,
    pub export: graphium::export::GraphDto,
    pub playground: Option<Playground>,
    pub tests: Vec<graphium::export::TestRun>,
}

impl UiGraph {
    pub fn from_export(export: graphium::export::GraphDto) -> Self {
        Self {
            id: export.id.clone(),
            name: export.name.clone(),
            export,
            playground: None,
            tests: Vec::new(),
        }
    }

    pub fn from_provider<
        G: graphium::GraphPlayground
            + graphium::GraphUiTests
            + ::serde::Serialize
            + ::core::default::Default
            + 'static,
    >() -> Self {
        let export: graphium::export::GraphDto = ::serde_json::from_value(
            ::serde_json::to_value(G::default()).expect("serialize graph"),
        )
        .expect("deserialize graph dto");
        let tests = <G as graphium::GraphUiTests>::graphium_ui_tests();
        let id = export.id.clone();
        Self {
            id,
            name: export.name.clone(),
            export,
            playground: Some(Playground {
                supported: G::PLAYGROUND_SUPPORTED,
                schema: G::playground_schema(),
                run: G::playground_run,
            }),
            tests,
        }
    }
}

pub fn graph<
    G: graphium::GraphPlayground
        + graphium::GraphUiTests
        + ::serde::Serialize
        + ::core::default::Default
        + 'static,
>() -> UiGraph {
    UiGraph::from_provider::<G>()
}

pub type ConfiguredGraph = UiGraph;
