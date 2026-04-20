use std::sync::Arc;

use askama::Template;
use axum::{extract::State, response::Html};

use crate::state::{graph::ConfiguredGraph, AppState};

#[derive(Template)]
#[template(path = "pages/home.html")]
pub struct HomeTemplate<'a> {
    pub title: &'a str,
    pub graphs: &'a [ConfiguredGraph],
}

pub async fn home(State(state): State<Arc<AppState>>) -> Html<String> {
    let template = HomeTemplate {
        title: "Graphium Home",
        graphs: &state.ordered,
    };

    Html(template.render().unwrap())
}
