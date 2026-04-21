use askama::Template;

use crate::server::ListQuery;
use crate::state::AppState;

#[derive(Template)]
#[template(path = "pages/graphs.html")]
pub(crate) struct GraphsTemplate {
    pub(crate) title: String,
    pub(crate) active: String,
    pub(crate) items: Vec<GraphItem>,
    pub(crate) current_page: usize,
    pub(crate) total_pages: usize,
    pub(crate) sort: String,
    pub(crate) search: String,
}

#[derive(Clone)]
pub(crate) struct GraphItem {
    pub(crate) name: String,
    pub(crate) url: String,
}

pub(crate) fn graphs_page_html(state: &AppState, query: ListQuery) -> String {
    let mut items: Vec<GraphItem> = state
        .graphs
        .ordered
        .iter()
        .map(|g| GraphItem {
            name: g.name.clone(),
            url: format!("/graph/{}", g.id),
        })
        .collect();

    if let Some(ref s) = query.search {
        items.retain(|i| i.name.to_lowercase().contains(&s.to_lowercase()));
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

    GraphsTemplate {
        title: "Graphs | Graphium UI".to_string(),
        active: "graphs".to_string(),
        items,
        current_page: page,
        total_pages,
        sort: query.sort.unwrap_or("asc".to_string()),
        search: query.search.unwrap_or("".to_string()),
    }
    .render()
    .expect("render graphs")
}