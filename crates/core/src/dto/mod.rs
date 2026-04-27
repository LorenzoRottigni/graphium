//! DTOs exported by macro-generated graphs/nodes.
//!
//! The dto module is consumed by tools (e.g. graphium-ui) and by the macro
//! expansion code itself (graph + node metadata, tests, playground, etc.).

pub mod ctx;
pub mod graph;
pub mod io;
pub mod node;
pub mod playground;
pub mod test;

pub use ctx::*;
pub use graph::*;
pub use io::*;
pub use node::*;
pub use playground::*;
pub use test::*;

pub const EXPORT_SCHEMA_VERSION: u32 = 2;

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphiumBundleDto {
    pub schema_version: u32,
    pub graphs: Vec<GraphDto>,
    pub nodes: Vec<NodeDto>,
}

impl GraphiumBundleDto {
    pub fn new() -> Self {
        Self {
            schema_version: EXPORT_SCHEMA_VERSION,
            graphs: Vec::new(),
            nodes: Vec::new(),
        }
    }

    pub fn from_graph_roots(roots: &[GraphDto]) -> Self {
        let mut bundle = Self::new();
        for graph in roots {
            bundle.insert_graph_recursive(graph);
        }
        bundle.dedupe();
        bundle
    }

    pub fn dedupe(&mut self) {
        use std::collections::HashSet;

        let mut seen_graphs: HashSet<String> = HashSet::new();
        self.graphs.retain(|g| seen_graphs.insert(g.id.clone()));

        let mut seen_nodes: HashSet<String> = HashSet::new();
        self.nodes.retain(|n| seen_nodes.insert(n.id.clone()));
    }

    fn insert_graph_recursive(&mut self, graph: &GraphDto) {
        self.graphs.push(graph.clone());
        self.nodes.extend(graph.nodes.iter().cloned());
        for sub in &graph.subgraphs {
            self.insert_graph_recursive(sub);
        }
    }
}

pub fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(msg) = payload.downcast_ref::<&'static str>() {
        return (*msg).to_string();
    }
    if let Some(msg) = payload.downcast_ref::<String>() {
        return msg.clone();
    }
    "panic while running test".to_string()
}

pub fn slugify(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut prev_dash = false;
    for ch in value.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}
