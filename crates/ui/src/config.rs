use std::net::SocketAddr;

use crate::{
    state::graph::{UiGraph, graph},
    util::default_bind,
};

#[derive(Clone)]
pub struct GraphiumUiConfig {
    pub bind: SocketAddr,
    pub prometheus_url: String,
    pub loki_url: String,
    pub tempo_url: String,
    pub graphs: Vec<UiGraph>,
}

impl GraphiumUiConfig {
    pub fn new(bind: SocketAddr, prometheus_url: impl Into<String>) -> Self {
        Self {
            bind,
            prometheus_url: prometheus_url.into(),
            loki_url: "http://127.0.0.1:3100".to_string(),
            tempo_url: "http://127.0.0.1:3200".to_string(),
            graphs: Vec::new(),
        }
    }

    pub fn from_graphs(prometheus_url: impl Into<String>, graphs: Vec<UiGraph>) -> Self {
        Self {
            bind: default_bind(),
            prometheus_url: prometheus_url.into(),
            loki_url: "http://127.0.0.1:3100".to_string(),
            tempo_url: "http://127.0.0.1:3200".to_string(),
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

    pub fn with_loki_url(mut self, loki_url: impl Into<String>) -> Self {
        self.loki_url = loki_url.into();
        self
    }

    pub fn with_tempo_url(mut self, tempo_url: impl Into<String>) -> Self {
        self.tempo_url = tempo_url.into();
        self
    }
}

impl Default for GraphiumUiConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            prometheus_url: "http://127.0.0.1:9090".to_string(),
            loki_url: "http://127.0.0.1:3100".to_string(),
            tempo_url: "http://127.0.0.1:3200".to_string(),
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
