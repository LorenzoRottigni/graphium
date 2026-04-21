use std::collections::HashMap;

use askama::Template;

use crate::server::ListQuery;
use crate::state::{test::{TestExecution, UiTest}, AppState};
use crate::util::normalize_symbol;

#[derive(Template)]
#[template(path = "pages/tests.html")]
pub(crate) struct TestsTemplate {
    pub(crate) title: &'static str,
    pub(crate) active: &'static str,
    pub(crate) tests: Vec<UiTest>,
    pub(crate) current_page: usize,
    pub(crate) total_pages: usize,
    pub(crate) sort: String,
    pub(crate) search: String,
}

pub(crate) fn tests_page_html(state: &AppState, query: ListQuery) -> String {
    let mut tests: Vec<UiTest> = state.tests.ordered.clone();

    if let Some(ref s) = query.search {
        tests.retain(|t| t.dto.name.to_lowercase().contains(&s.to_lowercase()));
    }

    tests.sort_by_key(|t| t.dto.name.clone());

    if query.sort.as_deref() == Some("desc") {
        tests.reverse();
    }

    let page_size = 20;
    let page = query.page.unwrap_or(1).max(1);
    let total = tests.len();
    let start = (page - 1) * page_size;
    let end = (start + page_size).min(total);
    let tests = tests[start..end].to_vec();
    let total_pages = (total + page_size - 1) / page_size;

    TestsTemplate {
        title: "Tests | Graphium UI",
        active: "tests",
        tests,
        current_page: page,
        total_pages,
        sort: query.sort.unwrap_or("asc".to_string()),
        search: query.search.unwrap_or("".to_string()),
    }
    .render()
    .expect("render tests")
}

#[derive(Clone)]
pub(crate) struct ParamView {
    pub(crate) name: String,
    pub(crate) is_bool: bool,
    pub(crate) checked: bool,
    pub(crate) input_type: String,
    pub(crate) value: String,
}

#[derive(Template)]
#[template(path = "pages/run_test.html")]
pub(crate) struct RunTestTemplate {
    pub(crate) title: &'static str,
    pub(crate) active: &'static str,

    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) kind: String,

    pub(crate) badge_color: String,
    pub(crate) badge_label: String,
    pub(crate) params: Vec<ParamView>,
    pub(crate) message: String,
}

pub(crate) fn run_test_page_html(
    test: &crate::state::test::UiTest,
    values: &HashMap<String, String>,
    result: Option<&TestExecution>,
) -> String {
    let badge_color = result
        .map(|r| if r.passed { "#1f9d55" } else { "#d64545" })
        .unwrap_or("#7a7a7a")
        .to_string();
    let badge_label = result
        .map(|r| if r.passed { "PASS" } else { "FAIL" })
        .unwrap_or("READY")
        .to_string();

    let params = test
        .schema
        .params
        .iter()
        .map(|param| {
            let raw_value = values.get(&param.name).cloned().unwrap_or_default();
            let checked = raw_value == "true" || raw_value == "1";
            let (is_bool, input_type) = match param.kind {
                graphium::export::TestParamKind::Bool => (true, "checkbox".to_string()),
                graphium::export::TestParamKind::Number => (false, "number".to_string()),
                graphium::export::TestParamKind::Text => (false, "text".to_string()),
            };
            ParamView {
                name: param.name.clone(),
                is_bool,
                checked,
                input_type,
                value: raw_value,
            }
        })
        .collect::<Vec<_>>();

    let message = result
        .map(|r| r.message.clone())
        .unwrap_or_else(|| "fill arguments and run".to_string());

    RunTestTemplate {
        title: "Run Test | Graphium UI",
        active: "tests",
        id: test.dto.id.clone(),
        name: normalize_symbol(&test.dto.name),
        kind: test.kind_label().to_string(),
        badge_color,
        badge_label,
        params,
        message,
    }
    .render()
    .expect("render run test template")
}
