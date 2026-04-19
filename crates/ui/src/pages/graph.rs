use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::sync::Arc;

use crate::http::AppHttpError;
use crate::layout::{render_page, LayoutContext};
use crate::mermaid::to_mermaid;
use crate::metrics::{fetch_metrics, fmt_metric};
use crate::state::{collect_graph_node_names, collect_graph_node_symbols, AppState, UiTest};
use crate::types::ConfiguredGraph;
use crate::util::{escape_label, escape_pre, normalize_symbol, slugify};

#[derive(Default, Clone)]
pub(crate) struct PlaygroundView {
    pub(crate) values: HashMap<String, String>,
    pub(crate) result: Option<Result<String, String>>,
}

pub(crate) fn shell_page_html(state: &AppState, selected_id: &str) -> String {
    let mut options = String::new();
    for candidate in &state.ordered {
        let selected = if candidate.id == selected_id {
            "selected"
        } else {
            ""
        };
        let _ = writeln!(
            options,
            r#"<option value="{}" {}>{}</option>"#,
            candidate.id, selected, candidate.name
        );
    }

    let header_extra = format!(
        r##"<form method="get" action="/graph/{selected_id}" @submit.prevent>
  <select id="graph-id"
    x-model="graphId"
    x-bind:hx-get="'/fragment/graph/' + encodeURIComponent(graphId)"
    hx-trigger="change"
    hx-target="#graph-content"
    hx-swap="innerHTML"
    hx-indicator="#loading"
  >{options}</select>
</form>"##,
        selected_id = escape_label(selected_id),
        options = options
    );

    let extra_head = r#"
  <script type="module">
    import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.esm.min.mjs';
    window.__graphiumMermaid = mermaid;
    mermaid.initialize({
      startOnLoad: false,
      theme: 'dark',
      securityLevel: 'loose',
      flowchart: { useMaxWidth: false }
    });
    window.__graphiumRenderMermaid = () => {
      try {
        if (window.__graphiumMermaid?.run) {
          window.__graphiumMermaid.run({ querySelector: '.mermaid' });
        } else if (window.__graphiumMermaid?.init) {
          window.__graphiumMermaid.init(undefined, document.querySelectorAll('.mermaid'));
        }
      } catch (_) {}
    };
    window.__graphiumRenderMermaid();
  </script>
  <script>
    document.addEventListener('htmx:afterSwap', function() {
      if (window.__graphiumRenderMermaid) window.__graphiumRenderMermaid();
    });
  </script>
"#;

    let main = format!(
        r##"<section id="graph-content"
  hx-get="/fragment/graph/{selected_id}"
  hx-trigger="load"
  hx-swap="innerHTML"
  hx-indicator="#loading"
></section>"##,
        selected_id = escape_label(selected_id)
    );

    let ctx = LayoutContext::graphs(state, selected_id)
        .set_header_extra_html(header_extra)
        .set_extra_head_html(extra_head.to_string());

    render_page("Graphium UI", ctx, main)
}

pub(crate) async fn render_graph_fragment(
    state: Arc<AppState>,
    id: String,
    playground_view: PlaygroundView,
) -> Result<String, AppHttpError> {
    let graph = state
        .by_id
        .get(&id)
        .ok_or_else(|| AppHttpError::not_found("graph not configured"))?;

    let linkable_graphs: HashSet<String> = state.by_id.keys().cloned().collect();
    let mermaid = to_mermaid(
        &graph.export.def,
        graph.playground.map(|p| p.schema.context),
        &linkable_graphs,
    );
    let metrics = fetch_metrics(&state, &graph.export.def.name).await;

    let count = fmt_metric(metrics.count);
    let errors = fmt_metric(metrics.errors);
    let success = fmt_metric(metrics.success);
    let fail = fmt_metric(metrics.fail);
    let p50 = fmt_metric(metrics.p50_seconds);
    let p95 = fmt_metric(metrics.p95_seconds);
    let graph_name_key = normalize_symbol(&graph.export.def.name);
    let node_symbols = collect_graph_node_symbols(&graph.export.def);

    let graph_scoped_tests: Vec<&UiTest> = state
        .tests_ordered
        .iter()
        .filter(|test| {
            matches!(test.kind, graphium::test_registry::TestKind::Graph)
                && normalize_symbol(&test.target) == graph_name_key
        })
        .collect();

    let node_scoped_tests: Vec<&UiTest> = state
        .tests_ordered
        .iter()
        .filter(|test| {
            matches!(test.kind, graphium::test_registry::TestKind::Node)
                && node_symbols.contains(&normalize_symbol(&test.target))
        })
        .collect();

    let graph_tests_widget = tests_widget_html("Graph Tests", &graph_scoped_tests);
    let node_tests_widget = tests_widget_html("Node Tests", &node_scoped_tests);
    let playground_widget = playground_widget_html(graph, &id, &playground_view);
    let nodes_widget = nodes_widget_html(&graph.export.def);
    let raw_schema_widget = raw_schema_widget_html(graph);

    Ok(format!(
        r#"<section class="card hero">
  <h3>Graph structure</h3>
  <div class="mermaid-scroll">
    <pre class="mermaid">{mermaid}</pre>
  </div>
</section>

<section class="below">
  <aside class="card">
    <h3>Prometheus metrics</h3>
    <p style="margin-top:0; opacity:.75;">Source: {prometheus}</p>
    <div class="metrics">
      <div class="metric"><div class="k">Executions</div><div class="v">{count}</div></div>
      <div class="metric"><div class="k">Errors</div><div class="v">{errors}</div></div>
      <div class="metric"><div class="k">Success</div><div class="v">{success}</div></div>
      <div class="metric"><div class="k">Fail</div><div class="v">{fail}</div></div>
      <div class="metric"><div class="k">P50 latency (s)</div><div class="v">{p50}</div></div>
      <div class="metric"><div class="k">P95 latency (s)</div><div class="v">{p95}</div></div>
    </div>
  </aside>
  <section class="side-stack">
    {nodes_widget}
    {raw_schema_widget}
    {playground_widget}
    <section class="tests-stack">
      {graph_tests_widget}
      {node_tests_widget}
    </section>
  </section>
</section>"#,
        mermaid = mermaid,
        prometheus = escape_label(&state.prometheus_base_url),
        count = count,
        errors = errors,
        success = success,
        fail = fail,
        p50 = p50,
        p95 = p95,
        nodes_widget = nodes_widget,
        raw_schema_widget = raw_schema_widget,
        playground_widget = playground_widget,
        graph_tests_widget = graph_tests_widget,
        node_tests_widget = node_tests_widget,
    ))
}

fn nodes_widget_html(def: &graphium::export::GraphDefDto) -> String {
    let node_names = collect_graph_node_names(def);
    if node_names.is_empty() {
        return r#"<article class="card"><h3>Nodes</h3><p class="muted" style="margin:.2rem 0;">No nodes.</p></article>"#.to_string();
    }

    let mut body = String::new();
    for name in node_names {
        let node_id = slugify(&normalize_symbol(&name));
        let _ = writeln!(
            body,
            r#"<div class="test-item">
  <span class="test-name" style="font-weight:700;">{}</span>
  <a class="test-run" href="/node/{}">Open</a>
</div>"#,
            escape_label(&normalize_symbol(&name)),
            escape_label(&node_id),
        );
    }

    format!(
        r#"<article class="card">
  <h3 style="margin-top:0;">Nodes</h3>
  {body}
</article>"#,
        body = body
    )
}

fn raw_schema_widget_html(graph: &ConfiguredGraph) -> String {
    let schema = read_source_span(graph.export.raw_span.as_ref())
        .map(|s| escape_pre(&s))
        .or_else(|| graph.export.raw_schema.as_deref().map(escape_pre))
        .unwrap_or_else(|| "Raw schema not available for this graph.".to_string());
    format!(
        r#"<article class="card">
  <h3 style="margin-top:0;">Raw schema</h3>
  <pre class="play-out" style="white-space:pre; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;">{schema}</pre>
</article>"#,
        schema = schema
    )
}

fn read_source_span(span: Option<&graphium::export::SourceSpanDto>) -> Option<String> {
    let span = span?;
    if span.start_line == 0 || span.end_line == 0 || span.end_line < span.start_line {
        return None;
    }
    let src = std::fs::read_to_string(&span.file).ok()?;
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
    if out.is_empty() { None } else { Some(out) }
}

fn tests_widget_html(title: &str, tests: &[&UiTest]) -> String {
    let mut body = String::new();
    if tests.is_empty() {
        body.push_str("<p style=\"opacity:.7; margin:.2rem 0;\">No tests linked.</p>");
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
                test.id
            );
        }
    }

    format!(
        r#"<article class="card">
  <h3>{}</h3>
  {}
</article>"#,
        escape_label(title),
        body
    )
}

fn playground_widget_html(graph: &ConfiguredGraph, id: &str, view: &PlaygroundView) -> String {
    let Some(playground) = graph.playground else {
        return r#"<article class="card"><h3>Playground</h3><p style="opacity:.7; margin:.2rem 0;">Not available for this graph.</p></article>"#.to_string();
    };

    let mut fields = String::new();
    for param in playground.schema.inputs {
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

    let outputs = if playground.schema.outputs.is_empty() {
        "Outputs: (none)".to_string()
    } else {
        let mut out = String::new();
        for (idx, param) in playground.schema.outputs.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push_str(param.name);
            out.push_str(": ");
            out.push_str(param.ty);
        }
        format!("Outputs: {out}")
    };

    let status = if playground.supported {
        format!(
            r#"<p style="margin:.2rem 0; opacity:.75;">Context: <code>{}</code> · {}</p>"#,
            escape_label(playground.schema.context),
            escape_label(&outputs)
        )
    } else {
        format!(
            r#"<p style="margin:.2rem 0; opacity:.75;">Playground disabled for this graph. Context: <code>{}</code> · {}</p>"#,
            escape_label(playground.schema.context),
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

    // For checkbox inputs, browsers only send the key when checked. Add a tiny
    // hint so users know unchecked = false.
    let hint = if playground
        .schema
        .inputs
        .iter()
        .any(|p| p.ty.trim() == "bool")
    {
        r#"<p style="margin:.2rem 0; opacity:.7;">Tip: unchecked bool inputs are treated as false.</p>"#
    } else {
        ""
    };

    format!(
        r##"<article class="card">
  <h3>Playground</h3>
  {status}
  {hint}
  <form method="post"
    action="/graph/{id}/playground/run"
    hx-post="/graph/{id}/playground/run"
    hx-target="#graph-content"
    hx-swap="innerHTML"
    hx-indicator="#loading"
  >
    {fields}
    <button type="submit" {disabled}>Run</button>
  </form>
  {result_html}
</article>"##,
        status = status,
        hint = hint,
        id = escape_label(id),
        fields = fields,
        disabled = if playground.supported { "" } else { "disabled" },
        result_html = result_html
    )
}
