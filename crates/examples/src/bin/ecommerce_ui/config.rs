use graphium_ui::{config::GraphiumUiConfig, graphs};

use crate::graphs::{
    CreateProductGraph, DeleteProductGraph, GetProductGraph, ListProductsGraph, UpdateProductGraph,
};

pub fn config() -> GraphiumUiConfig {
    GraphiumUiConfig {
        bind: std::env::var("GRAPHIUM_UI_BIND")
            .unwrap_or_else(|_| "127.0.0.1:4000".to_string())
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:4000".parse().expect("valid default bind")),
        prometheus_url: std::env::var("GRAPHIUM_PROMETHEUS_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9090".to_string()),
        loki_url: std::env::var("GRAPHIUM_LOKI_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:3100".to_string()),
        tempo_url: std::env::var("GRAPHIUM_TEMPO_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:3200".to_string()),
        graphs: graphs![
            CreateProductGraph,
            GetProductGraph,
            ListProductsGraph,
            UpdateProductGraph,
            DeleteProductGraph
        ],
        ..Default::default()
    }
}
