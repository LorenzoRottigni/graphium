use std::collections::HashMap;
use std::net::SocketAddr;

use graphium::GraphDef;

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

    pub fn with_graph<G: graphium::GraphPlayground + 'static>(mut self) -> Self {
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
    pub def: GraphDef,
    pub playground: Option<Playground>,
}

impl ConfiguredGraph {
    pub fn from_graph_def(def: GraphDef) -> Self {
        let id = slugify(def.name);
        Self {
            id,
            name: def.name.to_string(),
            def,
            playground: None,
        }
    }

    pub fn from_provider<G: graphium::GraphPlayground + 'static>() -> Self {
        let def = G::graph_def();
        let id = slugify(def.name);
        Self {
            id,
            name: def.name.to_string(),
            def,
            playground: Some(Playground {
                supported: G::PLAYGROUND_SUPPORTED,
                schema: G::playground_schema(),
                run: G::playground_run,
            }),
        }
    }
}

pub fn graph<G: graphium::GraphPlayground + 'static>() -> ConfiguredGraph {
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
