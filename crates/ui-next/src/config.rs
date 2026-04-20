use std::net::SocketAddr;

use crate::{
    state::graph::{graph, UiGraph},
    util::default_bind,
};

#[derive(Clone)]
pub struct GraphiumUiConfig {
    pub bind: SocketAddr,
    pub prometheus_url: String,
    pub graphs: Vec<UiGraph>,
}

impl GraphiumUiConfig {
    pub fn new(bind: SocketAddr, prometheus_url: impl Into<String>) -> Self {
        Self {
            bind,
            prometheus_url: prometheus_url.into(),
            graphs: Vec::new(),
        }
    }

    pub fn from_graphs(prometheus_url: impl Into<String>, graphs: Vec<UiGraph>) -> Self {
        Self {
            bind: default_bind(),
            prometheus_url: prometheus_url.into(),
            graphs,
        }
    }

    pub fn with_graph<
        G: graphium::GraphPlayground
            + graphium::GraphUiTests
            + ::serde::Serialize
            + ::core::default::Default
            + 'static,
    >(
        mut self,
    ) -> Self {
        self.graphs.push(graph::<G>());
        self
    }

    pub fn with_graphs(mut self, graphs: Vec<UiGraph>) -> Self {
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

pub fn graphium_ui_config(
    prometheus_url: impl Into<String>,
    graphs: Vec<UiGraph>,
) -> GraphiumUiConfig {
    GraphiumUiConfig::from_graphs(prometheus_url, graphs)
}
