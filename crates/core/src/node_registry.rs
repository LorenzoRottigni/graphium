use std::collections::HashMap;

use crate::{CtxAccess, PlaygroundSchema};

pub struct RegisteredNode {
    /// Stable identifier used for URLs.
    pub id: &'static str,
    /// Primary type path for the node (stringified).
    pub target: &'static str,
    /// Alternative type paths this node can be referenced with (stringified).
    pub aliases: &'static [&'static str],
    /// A short label suitable for UI display.
    pub label: &'static str,
    /// Absolute-ish file path where the node was declared.
    pub file: &'static str,
    pub start_line: u32,
    pub end_line: u32,
    pub ctx_access: CtxAccess,
    /// Prometheus label for the `graph` dimension in node metrics.
    pub metrics_graph: &'static str,
    /// Prometheus label for the `node` dimension in node metrics.
    pub metrics_node: &'static str,
    pub playground_supported: bool,
    pub playground_schema: PlaygroundSchema,
    pub playground_run: fn(&HashMap<String, String>) -> Result<String, String>,
}

inventory::collect!(RegisteredNode);

pub fn registered_nodes() -> Vec<&'static RegisteredNode> {
    inventory::iter::<RegisteredNode>.into_iter().collect()
}

pub fn node_by_id(id: &str) -> Option<&'static RegisteredNode> {
    registered_nodes().into_iter().find(|node| node.id == id)
}

pub fn node_by_target(target: &str) -> Option<&'static RegisteredNode> {
    registered_nodes()
        .into_iter()
        .find(|node| match_target(node, target))
}

fn normalize_target(value: &str) -> String {
    value.replace(' ', "").replace('\n', "")
}

fn last_segment(value: &str) -> String {
    let cleaned = value.replace(' ', "").replace('\n', "");
    cleaned.rsplit("::").next().unwrap_or(&cleaned).to_string()
}

fn match_target(node: &RegisteredNode, target: &str) -> bool {
    let normalized = normalize_target(target);
    if normalize_target(node.target) == normalized {
        return true;
    }
    if node
        .aliases
        .iter()
        .any(|alias| normalize_target(alias) == normalized)
    {
        return true;
    }

    // Fallback: match on the last segment so `MyNode` can resolve even if a
    // graph uses `some::path::MyNode`.
    let target_last = last_segment(target);
    last_segment(node.target) == target_last
        || node
            .aliases
            .iter()
            .any(|alias| last_segment(alias) == target_last)
}
