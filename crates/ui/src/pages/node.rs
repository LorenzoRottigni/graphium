use std::fmt::Write as _;
use std::sync::Arc;

use crate::http::AppHttpError;
use crate::layout::{render_page, LayoutContext};
use crate::metrics::{fetch_node_metrics, fmt_metric};
use crate::state::{AppState, UiNode, UiTest};
use crate::util::{escape_label, escape_pre, normalize_symbol};

pub(crate) async fn node_page_html(
    state: Arc<AppState>,
    node_id: String,
) -> Result<String, AppHttpError> {
    let node = state
        .nodes_by_id
        .get(&node_id)
        .ok_or_else(|| AppHttpError::not_found("node not registered"))?;

    let source = read_source_snippet(node);

    let tests: Vec<&UiTest> = state
        .tests_ordered
        .iter()
        .filter(|test| {
            matches!(test.kind, graphium::test_registry::TestKind::Node)
                && normalize_symbol(&test.target) == normalize_symbol(&node.dto.target)
        })
        .collect();

    let metrics =
        fetch_node_metrics(&state, &node.dto.metrics_graph, &node.dto.metrics_node).await;

    let metrics_widget = node_metrics_widget(node, &metrics);
    let tests_widget = node_tests_widget(&tests);
    let code_widget = node_code_widget(node, &source);

    let main = format!(
        r#"<section class="card">
  <h2 style="margin-top:0;">Node: <code>{label}</code></h2>
  <p class="muted" style="margin-top:.2rem;">Target: <code>{target}</code></p>
  <p class="muted" style="margin-top:.2rem;">Context access: <code>{ctx_access}</code></p>
</section>

<section class="below">
  <section class="side-stack">
    {metrics_widget}
    {tests_widget}
  </section>
  <section class="side-stack">
    {code_widget}
  </section>
</section>"#,
        label = escape_label(&node.dto.label),
        target = escape_label(&node.dto.target),
        ctx_access = escape_label(ctx_access_label(node.dto.ctx_access)),
        metrics_widget = metrics_widget,
        tests_widget = tests_widget,
        code_widget = code_widget
    );

    Ok(render_page(
        &format!("Node: {} | Graphium UI", node.dto.label),
        LayoutContext::dashboard(),
        main,
    ))
}

fn node_code_widget(node: &UiNode, snippet: &SourceSnippet) -> String {
    let header = if snippet.available {
        let file = node
            .dto
            .source
            .as_ref()
            .map(|s| s.file.as_str())
            .unwrap_or("unknown");
        let start = node.dto.source.as_ref().map(|s| s.start_line).unwrap_or(0);
        let end = node.dto.source.as_ref().map(|s| s.end_line).unwrap_or(0);
        format!(
            r#"<p class="muted" style="margin:.2rem 0;">{file} · lines {start}–{end}</p>"#,
            file = escape_label(file),
            start = start,
            end = end
        )
    } else {
        let extra = node.dto.source.as_ref().map(|s| {
            format!(
                r#" <span style="opacity:.75;">({file}:{start}-{end})</span>"#,
                file = escape_label(&s.file),
                start = s.start_line,
                end = s.end_line
            )
        });
        format!(
            r#"<p class="muted" style="margin:.2rem 0;">Source not available.{}</p>"#,
            extra.unwrap_or_default()
        )
    };

    let code = if snippet.available {
        escape_pre(&snippet.text)
    } else {
        "".to_string()
    };

    format!(
        r#"<article class="card">
  <h3 style="margin-top:0;">Code</h3>
  {header}
  <pre class="play-out"><code class="language-rust" style="white-space:pre;">{code}</code></pre>
</article>"#,
        header = header,
        code = code
    )
}

fn node_metrics_widget(node: &UiNode, metrics: &crate::metrics::NodeMetricsView) -> String {
    let count = fmt_metric(metrics.count);
    let errors = fmt_metric(metrics.errors);
    let success = fmt_metric(metrics.success);
    let fail = fmt_metric(metrics.fail);
    let p50 = fmt_metric(metrics.p50_seconds);
    let p95 = fmt_metric(metrics.p95_seconds);

    format!(
        r#"<article class="card">
  <h3 style="margin-top:0;">Metrics</h3>
  <p class="muted" style="margin-top:0;">
    Labels: graph=<code>{g}</code> · node=<code>{n}</code>
  </p>
  <div class="metrics">
    <div class="metric"><div class="k">Executions</div><div class="v">{count}</div></div>
    <div class="metric"><div class="k">Errors</div><div class="v">{errors}</div></div>
    <div class="metric"><div class="k">Success</div><div class="v">{success}</div></div>
    <div class="metric"><div class="k">Fail</div><div class="v">{fail}</div></div>
    <div class="metric"><div class="k">P50 latency (s)</div><div class="v">{p50}</div></div>
    <div class="metric"><div class="k">P95 latency (s)</div><div class="v">{p95}</div></div>
  </div>
</article>"#,
        g = escape_label(&node.dto.metrics_graph),
        n = escape_label(&node.dto.metrics_node),
        count = count,
        errors = errors,
        success = success,
        fail = fail,
        p50 = p50,
        p95 = p95
    )
}

fn node_tests_widget(tests: &[&UiTest]) -> String {
    let mut body = String::new();
    if tests.is_empty() {
        body.push_str("<p class=\"muted\" style=\"margin:.2rem 0;\">No tests linked.</p>");
    } else {
        for test in tests {
            let _ = writeln!(
                body,
                r#"<div class="test-item">
  <span class="test-target">{}</span>
  <span class="test-name">{}</span>
  <a class="test-run" href="/tests/run/{}">Run</a>
</div>"#,
                escape_label(&test.target),
                escape_label(&test.name),
                escape_label(&test.id)
            );
        }
    }

    format!(
        r#"<article class="card">
  <h3 style="margin-top:0;">Tests</h3>
  {body}
</article>"#,
        body = body
    )
}

fn ctx_access_label(access: graphium::export::CtxAccessDto) -> &'static str {
    match access {
        graphium::export::CtxAccessDto::None => "none",
        graphium::export::CtxAccessDto::Ref => "&",
        graphium::export::CtxAccessDto::Mut => "&mut",
    }
}

struct SourceSnippet {
    available: bool,
    text: String,
}

fn read_source_snippet(node: &UiNode) -> SourceSnippet {
    let Some(span) = node.dto.source.as_ref() else {
        return SourceSnippet {
            available: false,
            text: String::new(),
        };
    };
    if span.start_line == 0 || span.end_line == 0 || span.end_line < span.start_line {
        return SourceSnippet {
            available: false,
            text: String::new(),
        };
    }

    let Ok(src) = std::fs::read_to_string(&span.file) else {
        return SourceSnippet {
            available: false,
            text: String::new(),
        };
    };

    let mut out = String::new();
    for (idx, line) in src.lines().enumerate() {
        let line_no = (idx + 1) as u32;
        if line_no < span.start_line {
            continue;
        }
        if line_no > span.end_line {
            break;
        }
        let _ = writeln!(out, "{line}");
    }

    SourceSnippet {
        available: !out.is_empty(),
        text: out,
    }
}
