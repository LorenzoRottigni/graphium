use crate::{ConfiguredGraph, GraphiumUiConfig};

pub fn graphium_ui_config(
    prometheus_url: impl Into<String>,
    graphs: Vec<ConfiguredGraph>,
) -> GraphiumUiConfig {
    GraphiumUiConfig::from_graphs(prometheus_url, graphs)
}
