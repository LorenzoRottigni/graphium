use std::collections::HashMap;
use std::net::SocketAddr;

use crate::util::{default_bind, slugify};

#[derive(Clone)]
pub struct GraphiumUiConfig {
    pub bind: SocketAddr,
    pub prometheus_url: String,
    pub graphs: Vec<ConfiguredGraph>,
}

impl GraphiumUiConfig {
    pub fn new(bind: SocketAddr, prometheus_url: impl Into<String>) -> Self {
        Self {
            bind,
            prometheus_url: prometheus_url.into(),
            graphs: Vec::new(),
        }
    }

    pub fn from_graphs(prometheus_url: impl Into<String>, graphs: Vec<ConfiguredGraph>) -> Self {
        Self {
            bind: default_bind(),
            prometheus_url: prometheus_url.into(),
            graphs,
        }
    }

    pub fn with_graph<G: graphium::GraphPlayground + ::serde::Serialize + ::core::default::Default + 'static>(
        mut self,
    ) -> Self {
        self.graphs.push(graph::<G>());
        self
    }

    pub fn with_graphs(mut self, graphs: Vec<ConfiguredGraph>) -> Self {
        self.graphs = graphs;
        self
    }
}

impl Default for GraphiumUiConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            prometheus_url: "http://127.0.0.1:9090".to_string(),
            graphs: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct ConfiguredGraph {
    pub id: String,
    pub name: String,
    pub export: graphium::export::GraphDto,
    pub playground: Option<Playground>,
}

impl ConfiguredGraph {
    pub fn from_export(export: graphium::export::GraphDto) -> Self {
        Self {
            id: export.id.clone(),
            name: export.name.clone(),
            export,
            playground: None,
        }
    }

    pub fn from_export_def(def: graphium::export::GraphDefDto) -> Self {
        let id = slugify(&def.name);
        Self {
            id: id.clone(),
            name: def.name.clone(),
            export: graphium::export::GraphDto {
                id,
                name: def.name.clone(),
                schema: None,
                def,
                raw_schema: None,
                raw_span: None,
                nodes: Vec::new(),
                subgraphs: Vec::new(),
                playground: None,
            },
            playground: None,
        }
    }

    pub fn from_provider<
        G: graphium::GraphPlayground + ::serde::Serialize + ::core::default::Default + 'static,
    >(
    ) -> Self {
        let export: graphium::export::GraphDto =
            ::serde_json::from_value(::serde_json::to_value(G::default()).expect("serialize graph"))
                .expect("deserialize graph dto");
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
        }
    }
}

pub fn graph<G: graphium::GraphPlayground + ::serde::Serialize + ::core::default::Default + 'static>(
) -> ConfiguredGraph {
    ConfiguredGraph::from_provider::<G>()
}

#[derive(Clone, Copy)]
pub struct Playground {
    pub(crate) supported: bool,
    pub(crate) schema: graphium::PlaygroundSchema,
    pub(crate) run: fn(&HashMap<String, String>) -> Result<String, String>,
}

#[derive(Debug)]
pub enum UiError {
    EmptyGraphs,
    Bind(std::io::Error),
}

impl std::fmt::Display for UiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiError::EmptyGraphs => write!(f, "graphium-ui config requires at least one graph"),
            UiError::Bind(err) => write!(f, "failed to bind graphium-ui server: {err}"),
        }
    }
}

impl std::error::Error for UiError {}
