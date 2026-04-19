use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

use crate::http::AppHttpError;
use crate::layout::{render_page, LayoutContext};
use crate::metrics::{fetch_node_metrics, fmt_metric};
use crate::state::{AppState, UiNode, UiTest};
use crate::util::{escape_label, normalize_symbol};

#[derive(serde::Deserialize, Default)]
pub(crate) struct NodeQuery {
    pub(crate) graph: Option<String>,
}

#[derive(Default, Clone)]
pub(crate) struct NodePlaygroundView {
    pub(crate) values: HashMap<String, String>,
    pub(crate) result: Option<Result<String, String>>,
}

pub(crate) async fn node_page_html(
    state: Arc<AppState>,
    node_id: String,
    query: NodeQuery,
    playground_view: NodePlaygroundView,
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
                && normalize_symbol(&test.target) == normalize_symbol(&node.target)
        })
        .collect();

    let graph_id = query.graph.as_deref();
    let metrics = fetch_node_metrics(&state, &node.metrics_graph, &node.metrics_node).await;

    let metrics_widget = node_metrics_widget(node, &metrics);
    let tests_widget = node_tests_widget(&tests);
    let playground_widget = playground_widget_html(node, graph_id, &playground_view);
    let code_widget = node_code_widget(node, &source);

    let main = format!(
        r#"<section class="card">
  <h2 style="margin-top:0;">Node: <code>{label}</code></h2>
  <p class="muted" style="margin-top:.2rem;">Target: <code>{target}</code></p>
  <p class="muted" style="margin-top:.2rem;">Context access: <code>{ctx_access}</code></p>
</section>

<section class="below">
  <section class="side-stack">
    {playground_widget}
    {metrics_widget}
    {tests_widget}
  </section>
  <section class="side-stack">
    {code_widget}
  </section>
</section>"#,
        label = escape_label(&node.label),
        target = escape_label(&node.target),
        ctx_access = escape_label(ctx_access_label(node.ctx_access)),
        playground_widget = playground_widget,
        metrics_widget = metrics_widget,
        tests_widget = tests_widget,
        code_widget = code_widget
    );

    Ok(render_page(
        &format!("Node: {} | Graphium UI", node.label),
        LayoutContext::dashboard(),
        main,
    ))
}

fn node_code_widget(node: &UiNode, snippet: &SourceSnippet) -> String {
    let header = if snippet.available {
        format!(
            r#"<p class="muted" style="margin:.2rem 0;">{file} · lines {start}–{end}</p>"#,
            file = escape_label(&node.file),
            start = node.start_line,
            end = node.end_line
        )
    } else {
        r#"<p class="muted" style="margin:.2rem 0;">Source not available.</p>"#.to_string()
    };

    let code = if snippet.available {
        escape_label(&snippet.text)
    } else {
        "".to_string()
    };

    format!(
        r#"<article class="card">
  <h3 style="margin-top:0;">Code</h3>
  {header}
  <pre class="play-out" style="white-space:pre; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;">{code}</pre>
</article>"#,
        header = header,
        code = code
    )
}

pub(crate) fn playground_widget_html(
    node: &UiNode,
    graph_id: Option<&str>,
    view: &NodePlaygroundView,
) -> String {
    let mut fields = String::new();
    for param in node.playground_schema.inputs {
        let value = view.values.get(param.name).cloned().unwrap_or_default();
        let checked = matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        );

        let _ = writeln!(
            fields,
            r#"<div class="play-field">
  <div class="play-label"><strong>{}</strong> <span style="opacity:.7;">({})</span></div>
  {}
</div>"#,
            escape_label(param.name),
            escape_label(param.ty),
            if param.ty.trim() == "bool" {
                format!(
                    r#"<label style="display:flex; align-items:center; gap:.5rem;">
  <input type="checkbox" name="{}" {}
  />
  <span style="opacity:.8;">true</span>
</label>"#,
                    escape_label(param.name),
                    if checked { "checked" } else { "" }
                )
            } else {
                format!(
                    r#"<input type="text" name="{}" value="{}" />"#,
                    escape_label(param.name),
                    escape_label(&value)
                )
            }
        );
    }

    let outputs = if node.playground_schema.outputs.is_empty() {
        "Outputs: (none)".to_string()
    } else {
        let mut out = String::new();
        for (idx, param) in node.playground_schema.outputs.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push_str(param.name);
            out.push_str(": ");
            out.push_str(param.ty);
        }
        format!("Outputs: {out}")
    };

    let status = if node.playground_supported {
        format!(
            r#"<p style="margin:.2rem 0; opacity:.75;">Context: <code>{}</code> · {}</p>"#,
            escape_label(node.playground_schema.context),
            escape_label(&outputs)
        )
    } else {
        format!(
            r#"<p style="margin:.2rem 0; opacity:.75;">Playground disabled for this node. Context: <code>{}</code> · {}</p>"#,
            escape_label(node.playground_schema.context),
            escape_label(&outputs)
        )
    };

    let result_html = match &view.result {
        None => "".to_string(),
        Some(Ok(value)) => format!(
            r#"<div style="margin-top:.7rem;"><div class="play-label">Result</div><pre class="play-out">{}</pre></div>"#,
            escape_label(value)
        ),
        Some(Err(err)) => format!(
            r#"<div style="margin-top:.7rem;"><div class="play-label">Error</div><pre class="play-out" style="border:1px solid #7f1d1d; background:#2a0f13;">{}</pre></div>"#,
            escape_label(err)
        ),
    };

    let hint = if node
        .playground_schema
        .inputs
        .iter()
        .any(|p| p.ty.trim() == "bool")
    {
        r#"<p style="margin:.2rem 0; opacity:.7;">Tip: unchecked bool inputs are treated as false.</p>"#
    } else {
        ""
    };

    let graph_q = graph_id
        .map(|g| format!("?graph={}", escape_label(g)))
        .unwrap_or_default();
    format!(
        r##"<article class="card" id="node-playground">
  <h3 style="margin-top:0;">Playground</h3>
  {status}
  {hint}
  <form method="post"
    action="/node/{id}/playground/run{graph_q}"
    hx-post="/node/{id}/playground/run{graph_q}"
    hx-target="#node-playground"
    hx-swap="outerHTML"
    hx-indicator="#loading"
  >
    {fields}
    <button type="submit" {disabled}>Run</button>
  </form>
  {result_html}
</article>"##,
        status = status,
        hint = hint,
        id = escape_label(&node.id),
        graph_q = graph_q,
        fields = fields,
        disabled = if node.playground_supported {
            ""
        } else {
            "disabled"
        },
        result_html = result_html
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
        g = escape_label(&node.metrics_graph),
        n = escape_label(&node.metrics_node),
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

fn ctx_access_label(access: graphium::CtxAccess) -> &'static str {
    match access {
        graphium::CtxAccess::None => "none",
        graphium::CtxAccess::Ref => "&",
        graphium::CtxAccess::Mut => "&mut",
    }
}

struct SourceSnippet {
    available: bool,
    text: String,
}

fn read_source_snippet(node: &UiNode) -> SourceSnippet {
    if node.start_line == 0 || node.end_line == 0 || node.end_line < node.start_line {
        return SourceSnippet {
            available: false,
            text: String::new(),
        };
    }

    let Ok(src) = std::fs::read_to_string(&node.file) else {
        return SourceSnippet {
            available: false,
            text: String::new(),
        };
    };

    let start = node.start_line.saturating_sub(1) as usize;
    let end = node.end_line as usize;
    let mut out = String::new();
    for (idx, line) in src.lines().enumerate() {
        if idx < start {
            continue;
        }
        if idx >= end {
            break;
        }
        let _ = writeln!(out, "{line}");
    }

    SourceSnippet {
        available: !out.is_empty(),
        text: out,
    }
}
