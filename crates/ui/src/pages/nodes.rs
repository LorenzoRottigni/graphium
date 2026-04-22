use askama::Template;

use crate::server::ListQuery;
use crate::state::AppState;

#[derive(Template)]
#[template(path = "pages/nodes.html")]
pub(crate) struct NodesTemplate {
    pub(crate) title: String,
    pub(crate) active: String,
    pub(crate) items: Vec<NodeItem>,
    pub(crate) current_page: usize,
    pub(crate) total_pages: usize,
    pub(crate) sort: String,
    pub(crate) search: String,
    pub(crate) tag: String,
    pub(crate) deprecated: String,
}

#[derive(Clone)]
pub(crate) struct NodeItem {
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) graph_name: String,
    pub(crate) graph_url: String,
    pub(crate) tags: Vec<String>,
    pub(crate) deprecated: bool,
}

pub(crate) fn nodes_page_html(state: &AppState, query: ListQuery) -> String {
    let mut items: Vec<NodeItem> = state
        .nodes
        .ordered
        .iter()
        .map(|n| NodeItem {
            name: n.dto.label.clone(),
            url: format!("/node/{}", n.dto.id),
            graph_name: state.graphs.by_id[&n.graph_id].name.clone(),
            graph_url: format!("/graph/{}", n.graph_id),
            tags: n.dto.tags.clone(),
            deprecated: n.dto.deprecated,
        })
        .collect();

    if let Some(ref s) = query.search {
        items.retain(|i| i.name.to_lowercase().contains(&s.to_lowercase()));
    }
    if let Some(ref tag) = query.tag {
        let tag = tag.trim();
        if !tag.is_empty() {
            items.retain(|i| i.tags.iter().any(|t| t == tag));
        }
    }
    match query.deprecated.as_deref() {
        Some("true") => items.retain(|i| i.deprecated),
        Some("false") => items.retain(|i| !i.deprecated),
        _ => {}
    }

    items.sort_by_key(|i| i.name.clone());

    if query.sort.as_deref() == Some("desc") {
        items.reverse();
    }

    let page_size = 20;
    let page = query.page.unwrap_or(1).max(1);
    let total = items.len();
    let start = (page - 1) * page_size;
    let end = (start + page_size).min(total);
    let items = items[start..end].to_vec();
    let total_pages = (total + page_size - 1) / page_size;

    NodesTemplate {
        title: "Nodes | Graphium UI".to_string(),
        active: "nodes".to_string(),
        items,
        current_page: page,
        total_pages,
        sort: query.sort.unwrap_or("asc".to_string()),
        search: query.search.unwrap_or("".to_string()),
        tag: query.tag.unwrap_or_default(),
        deprecated: query.deprecated.unwrap_or_default(),
    }
    .render()
    .expect("render nodes")
}
