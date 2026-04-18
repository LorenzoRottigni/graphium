mod http;
mod layout;
mod mermaid;
mod metrics;
mod pages;
mod server;
mod state;
mod types;
mod util;

pub mod config;

pub use crate::server::serve;
pub use crate::types::{graph, ConfiguredGraph, GraphiumUiConfig, Playground, UiError};

#[macro_export]
macro_rules! graphs {
    ($($graph:path),+ $(,)?) => {{
        vec![
            $(
                $crate::ConfiguredGraph::from_provider::<$graph>()
            ),+
        ]
    }};
}
