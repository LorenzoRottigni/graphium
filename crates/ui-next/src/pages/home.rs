use std::sync::Arc;

use askama::Template;
use axum::{extract::State, response::Html};

use crate::state::AppState;

#[derive(Template)]
#[template(path = "pages/home.html")]
pub struct HomeTemplate {
    pub title: String,
}

pub async fn home(State(_state): State<Arc<AppState>>) -> Html<String> {
    let template = HomeTemplate {
        title: "Graphium Home".to_string(),
    };

    Html(template.render().unwrap())
}
