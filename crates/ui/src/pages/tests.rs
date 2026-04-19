use std::fmt::Write as _;

use crate::layout::{render_page, LayoutContext};
use crate::state::{AppState, TestExecution, UiTest};
use crate::util::escape_label;

pub(crate) fn tests_page_html(state: &AppState) -> String {
    let mut cards = String::new();
    if state.tests_ordered.is_empty() {
        cards.push_str("<p class=\"muted\">No tests registered.</p>");
    } else {
        for test in &state.tests_ordered {
            let _ = writeln!(
                cards,
                r#"<div class="test-card">
  <div class="kind">{}</div>
  <div class="name">{}</div>
  <div class="target">{}</div>
  <a class="run" href="/tests/run/{}">Run</a>
</div>"#,
                test.kind_label(),
                escape_label(&test.dto.name),
                escape_label(&test.dto.target),
                escape_label(&test.dto.id)
            );
        }
    }

    let main = format!(
        r#"<section class="card">
  <h2 style="margin-top:0;">Tests</h2>
  <section style="display:grid; grid-template-columns: 1fr; gap:.8rem;">
    {cards}
  </section>
</section>"#,
        cards = cards
    );

    render_page("Tests | Graphium UI", LayoutContext::tests(state), main)
}

pub(crate) fn run_test_page_html(
    state: &AppState,
    test: &UiTest,
    values: &std::collections::HashMap<String, String>,
    result: Option<&TestExecution>,
) -> String {
    let badge_color = result
        .map(|r| if r.passed { "#1f9d55" } else { "#d64545" })
        .unwrap_or("#7a7a7a");
    let badge_label = result
        .map(|r| if r.passed { "PASS" } else { "FAIL" })
        .unwrap_or("READY");

    let mut args_form = String::new();
    if !test.schema.params.is_empty() {
        let mut fields = String::new();
        for param in &test.schema.params {
            let name = escape_label(&param.name);
            let raw_value = values.get(&param.name).cloned().unwrap_or_default();
            let value = escape_label(&raw_value);
            match param.kind {
                graphium::export::TestParamKind::Bool => {
                    let checked = if raw_value == "true" || raw_value == "1" {
                        "checked"
                    } else {
                        ""
                    };
                    let _ = writeln!(
                        fields,
                        r#"<label class="play-row" style="gap:.8rem;">
  <span style="min-width:160px;">{name}</span>
  <input type="hidden" name="{name_attr}" value="false" />
  <input type="checkbox" name="{name_attr}" value="true" {checked} />
</label>"#,
                        name = name,
                        name_attr = escape_label(&param.name),
                        checked = checked
                    );
                }
                graphium::export::TestParamKind::Number => {
                    let _ = writeln!(
                        fields,
                        r#"<label class="play-row">
  <span style="min-width:160px;">{name}</span>
  <input class="play-in" type="number" name="{name_attr}" value="{value}" />
</label>"#,
                        name = name,
                        name_attr = escape_label(&param.name),
                        value = value
                    );
                }
                graphium::export::TestParamKind::Text => {
                    let _ = writeln!(
                        fields,
                        r#"<label class="play-row">
  <span style="min-width:160px;">{name}</span>
  <input class="play-in" type="text" name="{name_attr}" value="{value}" />
</label>"#,
                        name = name,
                        name_attr = escape_label(&param.name),
                        value = value
                    );
                }
            }
        }

        args_form = format!(
            r#"<h3 style="margin-top:1.2rem;">Arguments</h3>
<form method="post" action="/tests/run/{id}">
  <section class="play-grid" style="margin-top:.5rem;">
    {fields}
  </section>
  <button class="btn" style="margin-top:.9rem;">Run</button>
</form>"#,
            id = escape_label(&test.dto.id),
            fields = fields
        );
    }

    let main = format!(
        r#"<section class="card" style="max-width:760px; margin:0 auto;">
  <h2 style="margin-top:0;">{name}</h2>
  <p><span style="display:inline-block; padding:.3rem .55rem; border-radius: 999px; color:white; font-size:.82rem; font-weight:700; background:{badge_color};">{badge_label}</span>
     <small class="muted">({kind})</small></p>
  {args_form}
  <h3>Output</h3>
  <pre class="play-out">{message}</pre>
  <p><a href="/tests">Back to tests</a></p>
</section>"#,
        name = escape_label(&test.dto.name),
        kind = escape_label(test.kind_label()),
        message = escape_label(
            &result
                .map(|r| r.message.clone())
                .unwrap_or_else(|| "fill arguments and run".to_string()),
        ),
        badge_color = badge_color,
        badge_label = badge_label,
        args_form = args_form
    );

    render_page("Run Test | Graphium UI", LayoutContext::tests(state), main)
}
