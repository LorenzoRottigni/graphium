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
    result: &TestExecution,
) -> String {
    let badge_color = if result.passed { "#1f9d55" } else { "#d64545" };
    let badge_label = if result.passed { "PASS" } else { "FAIL" };

    let main = format!(
        r#"<section class="card" style="max-width:760px; margin:0 auto;">
  <h2 style="margin-top:0;">{name}</h2>
  <p><span style="display:inline-block; padding:.3rem .55rem; border-radius: 999px; color:white; font-size:.82rem; font-weight:700; background:{badge_color};">{badge_label}</span>
     <small class="muted">({kind})</small></p>
  <h3>Output</h3>
  <pre class="play-out">{message}</pre>
  <p><a href="/tests">Back to tests</a></p>
</section>"#,
        name = escape_label(&test.dto.name),
        kind = escape_label(test.kind_label()),
        message = escape_label(&result.message),
        badge_color = badge_color,
        badge_label = badge_label
    );

    render_page("Run Test | Graphium UI", LayoutContext::tests(state), main)
}
