use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::Form;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use graphium::{GraphDef, GraphStep};
use reqwest::Url;
use serde::Deserialize;

pub mod config;

#[derive(Clone)]
pub struct GraphiumUiConfig {
    pub bind: SocketAddr,
    pub prometheus_url: String,
    pub graphs: Vec<ConfiguredGraph>,
}

impl GraphiumUiConfig {
    pub fn new(bind: SocketAddr, prometheus_url: impl Into<String>) -> Self {
        Self {
            bind,
            prometheus_url: prometheus_url.into(),
            graphs: Vec::new(),
        }
    }

    pub fn from_graphs(prometheus_url: impl Into<String>, graphs: Vec<ConfiguredGraph>) -> Self {
        Self {
            bind: default_bind(),
            prometheus_url: prometheus_url.into(),
            graphs,
        }
    }

    pub fn with_graph<G: graphium::GraphPlayground + 'static>(mut self) -> Self {
        self.graphs.push(graph::<G>());
        self
    }

    pub fn with_graphs(mut self, graphs: Vec<ConfiguredGraph>) -> Self {
        self.graphs = graphs;
        self
    }
}

impl Default for GraphiumUiConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            prometheus_url: "http://127.0.0.1:9090".to_string(),
            graphs: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct ConfiguredGraph {
    pub id: String,
    pub name: String,
    pub def: GraphDef,
    pub playground: Option<Playground>,
}

impl ConfiguredGraph {
    pub fn from_graph_def(def: GraphDef) -> Self {
        let id = slugify(def.name);
        Self {
            id,
            name: def.name.to_string(),
            def,
            playground: None,
        }
    }

    pub fn from_provider<G: graphium::GraphPlayground + 'static>() -> Self {
        let def = G::graph_def();
        let id = slugify(def.name);
        Self {
            id,
            name: def.name.to_string(),
            def,
            playground: Some(Playground {
                supported: G::PLAYGROUND_SUPPORTED,
                schema: G::playground_schema(),
                run: G::playground_run,
            }),
        }
    }
}

pub fn graph<G: graphium::GraphPlayground + 'static>() -> ConfiguredGraph {
    ConfiguredGraph::from_provider::<G>()
}

#[derive(Clone, Copy)]
pub struct Playground {
    supported: bool,
    schema: graphium::PlaygroundSchema,
    run: fn(&HashMap<String, String>) -> Result<String, String>,
}

#[derive(Default, Clone)]
struct PlaygroundView {
    values: HashMap<String, String>,
    result: Option<Result<String, String>>,
}

#[derive(Clone)]
struct UiTest {
    pub id: String,
    pub name: String,
    pub kind: graphium::test_registry::TestKind,
    pub target: String,
    run: fn() -> Result<(), String>,
}

impl UiTest {
    fn kind_label(&self) -> &'static str {
        match self.kind {
            graphium::test_registry::TestKind::Node => "Node",
            graphium::test_registry::TestKind::Graph => "Graph",
        }
    }

    fn run(&self) -> TestExecution {
        match (self.run)() {
            Ok(()) => TestExecution {
                passed: true,
                message: "ok".to_string(),
            },
            Err(err) => TestExecution {
                passed: false,
                message: err,
            },
        }
    }
}

#[derive(Clone)]
struct TestExecution {
    passed: bool,
    message: String,
}

#[macro_export]
macro_rules! graphs {
    ($($graph:path),+ $(,)?) => {{
        vec![
            $(
                $crate::ConfiguredGraph::from_provider::<$graph>()
            ),+
        ]
    }};
}

#[derive(Clone)]
struct AppState {
    prometheus_base_url: String,
    client: reqwest::Client,
    ordered: Vec<ConfiguredGraph>,
    by_id: HashMap<String, ConfiguredGraph>,
    tests_ordered: Vec<UiTest>,
    tests_by_id: HashMap<String, UiTest>,
}

#[derive(Debug)]
pub enum UiError {
    EmptyGraphs,
    Bind(std::io::Error),
}

impl std::fmt::Display for UiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiError::EmptyGraphs => write!(f, "graphium-ui config requires at least one graph"),
            UiError::Bind(err) => write!(f, "failed to bind graphium-ui server: {err}"),
        }
    }
}

impl std::error::Error for UiError {}

pub async fn serve(config: GraphiumUiConfig) -> Result<(), UiError> {
    if config.graphs.is_empty() {
        return Err(UiError::EmptyGraphs);
    }

    let mut ordered = config.graphs;
    let mut by_id = ordered
        .iter()
        .cloned()
        .map(|g| (g.id.clone(), g))
        .collect::<HashMap<_, _>>();

    // Auto-register nested graphs so users can click into subgraphs.
    let mut discovered = HashMap::<String, GraphDef>::new();
    let mut visited = HashSet::<String>::new();
    for graph in &ordered {
        collect_nested_graph_defs(&graph.def, &mut discovered, &mut visited);
    }
    let mut discovered_defs: Vec<GraphDef> = discovered.into_values().collect();
    discovered_defs.sort_by_key(|def| def.name.to_string());
    for def in discovered_defs {
        let candidate = ConfiguredGraph::from_graph_def(def);
        if by_id.contains_key(&candidate.id) {
            continue;
        }
        by_id.insert(candidate.id.clone(), candidate.clone());
        ordered.push(candidate);
    }

    let tests_ordered: Vec<UiTest> = graphium::test_registry::registered_tests()
        .into_iter()
        .map(|test| UiTest {
            id: format!(
                "{}-{}-{}",
                match test.kind {
                    graphium::test_registry::TestKind::Node => "node",
                    graphium::test_registry::TestKind::Graph => "graph",
                },
                slugify(test.target),
                slugify(test.name)
            ),
            name: test.name.to_string(),
            kind: test.kind,
            target: test.target.to_string(),
            run: test.run,
        })
        .collect();

    let tests_by_id = tests_ordered
        .iter()
        .cloned()
        .map(|t| (t.id.clone(), t))
        .collect::<HashMap<_, _>>();

    let state = Arc::new(AppState {
        prometheus_base_url: config.prometheus_url,
        client: reqwest::Client::new(),
        ordered,
        by_id,
        tests_by_id,
        tests_ordered,
    });

    let app = Router::new()
        .route("/", get(home))
        .route("/select/:id", get(select_graph))
        .route("/graph/:id", get(graph_page))
        .route("/graph/:id/playground/run", post(run_playground))
        .route("/tests", get(tests_page))
        .route("/tests/run/:id", get(run_test_page))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(UiError::Bind)?;
    axum::serve(listener, app).await.map_err(UiError::Bind)
}

fn default_bind() -> SocketAddr {
    "127.0.0.1:4000"
        .parse()
        .expect("default graphium-ui bind must be a valid socket address")
}

async fn home(State(state): State<Arc<AppState>>) -> Html<String> {
    let mut options = String::new();
    for graph in &state.ordered {
        let _ = writeln!(
            options,
            r#"<option value="{}">{}</option>"#,
            graph.id, graph.name
        );
    }

    Html(format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Graphium UI</title>
  <style>
    body {{ font-family: ui-sans-serif, system-ui, -apple-system, sans-serif; margin: 2rem; background: #f7f9fc; color: #1c2733; }}
    .card {{ max-width: 700px; margin: 4rem auto; background: white; padding: 2rem; border-radius: 16px; box-shadow: 0 10px 25px rgba(0,0,0,.08); }}
    h1 {{ margin-top: 0; }}
    select, button {{ padding: .8rem 1rem; font-size: 1rem; border-radius: 10px; border: 1px solid #d5dce5; }}
    button {{ background: #0f7bff; color: white; border: none; cursor: pointer; margin-left: .7rem; }}
  </style>
</head>
<body>
  <main class="card">
    <h1>Graphium UI</h1>
    <p>Select a graph to inspect its structure and Prometheus metrics.</p>
    <p><a href="/tests">Open tests tab</a></p>
    <form method="get" id="graph-picker">
      <select id="graph-id">{options}</select>
      <button type="submit">Open graph</button>
    </form>
  </main>
  <script>
    document.getElementById('graph-picker').addEventListener('submit', function(e) {{
      e.preventDefault();
      const id = document.getElementById('graph-id').value;
      window.location.href = '/graph/' + encodeURIComponent(id);
    }});
  </script>
</body>
</html>"#
    ))
}

async fn select_graph(Path(id): Path<String>) -> Redirect {
    Redirect::to(&format!("/graph/{id}"))
}

async fn graph_page(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, AppHttpError> {
    render_graph_page(state, id, PlaygroundView::default()).await
}

async fn run_playground(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(values): Form<HashMap<String, String>>,
) -> Result<Html<String>, AppHttpError> {
    let graph = state
        .by_id
        .get(&id)
        .ok_or_else(|| AppHttpError::not_found("graph not configured"))?;

    let result = graph
        .playground
        .map(|pg| (pg.run)(&values))
        .unwrap_or_else(|| Err("playground not available for this graph".to_string()));

    render_graph_page(
        state,
        id,
        PlaygroundView {
            values,
            result: Some(result),
        },
    )
    .await
}

async fn render_graph_page(
    state: Arc<AppState>,
    id: String,
    playground_view: PlaygroundView,
) -> Result<Html<String>, AppHttpError> {
    let graph = state
        .by_id
        .get(&id)
        .ok_or_else(|| AppHttpError::not_found("graph not configured"))?;

    let linkable_graphs: HashSet<String> = state.by_id.keys().cloned().collect();
    let mermaid = to_mermaid(&graph.def, &linkable_graphs);
    let metrics = fetch_metrics(&state, graph.def.name).await;

    let mut options = String::new();
    for candidate in &state.ordered {
        let selected = if candidate.id == id { "selected" } else { "" };
        let _ = writeln!(
            options,
            r#"<option value="{}" {}>{}</option>"#,
            candidate.id, selected, candidate.name
        );
    }

    let count = fmt_metric(metrics.count);
    let errors = fmt_metric(metrics.errors);
    let success = fmt_metric(metrics.success);
    let fail = fmt_metric(metrics.fail);
    let p50 = fmt_metric(metrics.p50_seconds);
    let p95 = fmt_metric(metrics.p95_seconds);
    let graph_name_key = normalize_symbol(graph.def.name);
    let node_symbols = collect_graph_node_symbols(&graph.def);

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

    Ok(Html(format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{name} | Graphium UI</title>
  <style>
    body {{ font-family: ui-sans-serif, system-ui, -apple-system, sans-serif; margin: 1.2rem; background: #f7f9fc; color: #1c2733; }}
    header {{ display: flex; gap: .8rem; align-items: center; margin-bottom: 1rem; flex-wrap: wrap; }}
    h1 {{ margin: 0; font-size: 1.35rem; }}
    select, button {{ padding: .6rem .8rem; font-size: .95rem; border-radius: 10px; border: 1px solid #d5dce5; }}
    button {{ background: #0f7bff; color: white; border: none; cursor: pointer; }}
    .hero {{ min-height: 420px; }}
    .card {{ background: white; border-radius: 14px; box-shadow: 0 10px 20px rgba(0,0,0,.06); padding: 1rem; overflow: auto; }}
    .mermaid-scroll {{ overflow-x: auto; overflow-y: auto; padding-bottom: .35rem; }}
    .mermaid-scroll svg {{ max-width: none !important; }}
    pre.mermaid {{ margin: 0; }}
    .below {{ display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; margin-top: 1rem; align-items: start; }}
    .side-stack {{ display:grid; grid-template-columns: 1fr; gap: 1rem; }}
    .metrics {{ display: grid; grid-template-columns: 1fr 1fr; gap: .75rem; }}
    .metric {{ border: 1px solid #ebeff5; border-radius: 12px; padding: .65rem; }}
    .metric .k {{ font-size: .8rem; opacity: .75; }}
    .metric .v {{ font-size: 1rem; font-weight: 700; margin-top: .2rem; }}
    .tests-stack {{ display:grid; grid-template-columns: 1fr; gap: 1rem; }}
    .play-label {{ font-size: .84rem; opacity: .8; margin-top: .3rem; }}
    .play-field {{ display: grid; grid-template-columns: 1fr; gap: .4rem; margin: .55rem 0; }}
    .play-field input[type="text"] {{ padding: .55rem .65rem; border-radius: 10px; border: 1px solid #d5dce5; }}
    .play-out {{ background: #f2f6fb; border-radius: 10px; padding: .75rem; overflow: auto; }}
    .test-item {{ border: 1px solid #ebeff5; border-radius: 10px; padding: .55rem; display:flex; align-items:center; gap:.5rem; }}
    .test-target {{ font-size: .83rem; color:#5f7388; }}
    .test-name {{ font-size: .9rem; font-weight: 600; flex:1; }}
    .test-run {{ text-decoration: none; background: #0f7bff; color: white; border-radius: 8px; padding: .3rem .55rem; font-size: .84rem; }}
    @media (max-width: 960px) {{ .below {{ grid-template-columns: 1fr; }} }}
  </style>
  <script type="module">
    import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.esm.min.mjs';
    mermaid.initialize({{ startOnLoad: true, theme: 'default', securityLevel: 'loose' }});
  </script>
</head>
<body>
  <header>
    <h1>{name}</h1>
    <form method="get" id="graph-picker">
      <select id="graph-id">{options}</select>
      <button type="submit">Switch graph</button>
      <a href="/tests" style="margin-left:.4rem;">Tests</a>
      <a href="/" style="margin-left:.4rem;">Home</a>
    </form>
  </header>

  <section class="card hero">
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
      {playground_widget}
      <section class="tests-stack">
        {graph_tests_widget}
        {node_tests_widget}
      </section>
    </section>
  </section>

  <script>
    document.getElementById('graph-picker').addEventListener('submit', function(e) {{
      e.preventDefault();
      const next = document.getElementById('graph-id').value;
      window.location.href = '/graph/' + encodeURIComponent(next);
    }});
  </script>
</body>
</html>"#,
        name = graph.name,
        options = options,
        mermaid = mermaid,
        prometheus = state.prometheus_base_url,
        count = count,
        errors = errors,
        success = success,
        fail = fail,
        p50 = p50,
        p95 = p95,
        playground_widget = playground_widget,
        graph_tests_widget = graph_tests_widget,
        node_tests_widget = node_tests_widget,
    )))
}

async fn tests_page(State(state): State<Arc<AppState>>) -> Html<String> {
    let mut cards = String::new();
    if state.tests_ordered.is_empty() {
        cards.push_str("<p>No tests configured at GraphiumUiConfig level.</p>");
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
                test.name,
                test.target,
                test.id
            );
        }
    }

    Html(format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Tests | Graphium UI</title>
  <style>
    body {{ font-family: ui-sans-serif, system-ui, -apple-system, sans-serif; margin: 1.2rem; background: #f7f9fc; color: #1c2733; }}
    .container {{ max-width: 900px; margin: 0 auto; }}
    .actions a {{ margin-right: .8rem; }}
    .grid {{ display: grid; grid-template-columns: 1fr; gap: .8rem; margin-top: 1rem; }}
    .test-card {{ background: white; border-radius: 12px; padding: 1rem; border: 1px solid #e6ebf2; display: flex; align-items: center; gap: .8rem; flex-wrap: wrap; }}
    .kind {{ font-size: .78rem; text-transform: uppercase; letter-spacing: .04em; color: #47617a; background: #edf4fb; padding: .25rem .5rem; border-radius: 999px; }}
    .name {{ font-weight: 600; flex: 1; }}
    .target {{ font-size:.86rem; color:#5f7388; flex-basis: 100%; }}
    .run {{ text-decoration: none; background: #0f7bff; color: white; border-radius: 8px; padding: .45rem .7rem; }}
  </style>
</head>
<body>
  <main class="container">
    <h1>Tests</h1>
    <div class="actions">
      <a href="/">Home</a>
    </div>
    <section class="grid">{cards}</section>
  </main>
</body>
</html>"#
    ))
}

async fn run_test_page(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, AppHttpError> {
    let test = state
        .tests_by_id
        .get(&id)
        .ok_or_else(|| AppHttpError::not_found("test not configured"))?;
    let result = test.run();
    let badge_color = if result.passed { "#1f9d55" } else { "#d64545" };
    let badge_label = if result.passed { "PASS" } else { "FAIL" };

    Ok(Html(format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Run Test | Graphium UI</title>
  <style>
    body {{ font-family: ui-sans-serif, system-ui, -apple-system, sans-serif; margin: 1.2rem; background: #f7f9fc; color: #1c2733; }}
    .card {{ max-width: 760px; margin: 0 auto; background: white; border-radius: 14px; border: 1px solid #e6ebf2; padding: 1rem; }}
    .badge {{ display:inline-block; padding:.3rem .55rem; border-radius: 999px; color:white; font-size:.82rem; font-weight:700; background:{badge_color}; }}
    pre {{ background: #f2f6fb; border-radius: 8px; padding: .8rem; overflow: auto; }}
  </style>
</head>
<body>
  <main class="card">
    <h1>{name}</h1>
    <p><span class="badge">{badge_label}</span> <small>({kind})</small></p>
    <h3>Output</h3>
    <pre>{message}</pre>
    <p><a href="/tests">Back to tests</a> · <a href="/">Home</a></p>
  </main>
</body>
</html>"#,
        name = test.name,
        kind = test.kind_label(),
        message = escape_label(&result.message),
    )))
}

#[derive(Default)]
struct MetricsView {
    count: Option<f64>,
    errors: Option<f64>,
    success: Option<f64>,
    fail: Option<f64>,
    p50_seconds: Option<f64>,
    p95_seconds: Option<f64>,
}

async fn fetch_metrics(state: &AppState, graph_name: &str) -> MetricsView {
    let esc = graph_name.replace('"', "\\\"");
    let count_q = format!(r#"sum(graphium_graph_count_total{{graph=\"{esc}\"}})"#);
    let errors_q = format!(r#"sum(graphium_graph_errors_total{{graph=\"{esc}\"}})"#);
    let success_q = format!(r#"sum(graphium_graph_success_total{{graph=\"{esc}\"}})"#);
    let fail_q = format!(r#"sum(graphium_graph_fail_total{{graph=\"{esc}\"}})"#);
    let p50_q = format!(
        r#"histogram_quantile(0.5, sum(rate(graphium_graph_latency_seconds_bucket{{graph=\"{esc}\"}}[5m])) by (le))"#
    );
    let p95_q = format!(
        r#"histogram_quantile(0.95, sum(rate(graphium_graph_latency_seconds_bucket{{graph=\"{esc}\"}}[5m])) by (le))"#
    );

    let count = prometheus_query_scalar(&state.client, &state.prometheus_base_url, &count_q).await;
    let errors =
        prometheus_query_scalar(&state.client, &state.prometheus_base_url, &errors_q).await;
    let success =
        prometheus_query_scalar(&state.client, &state.prometheus_base_url, &success_q).await;
    let fail = prometheus_query_scalar(&state.client, &state.prometheus_base_url, &fail_q).await;
    let p50_seconds =
        prometheus_query_scalar(&state.client, &state.prometheus_base_url, &p50_q).await;
    let p95_seconds =
        prometheus_query_scalar(&state.client, &state.prometheus_base_url, &p95_q).await;

    MetricsView {
        count,
        errors,
        success,
        fail,
        p50_seconds,
        p95_seconds,
    }
}

fn fmt_metric(value: Option<f64>) -> String {
    match value {
        Some(v) => format!("{v:.4}"),
        None => "n/a".to_string(),
    }
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
            r#"<div style="margin-top:.7rem;"><div class="play-label">Error</div><pre class="play-out" style="border:1px solid #fecaca; background:#fff1f2;">{}</pre></div>"#,
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
        r#"<article class="card">
  <h3>Playground</h3>
  {status}
  {hint}
  <form method="post" action="/graph/{id}/playground/run">
    {fields}
    <button type="submit" {disabled}>Run</button>
  </form>
  {result_html}
</article>"#,
        status = status,
        hint = hint,
        id = escape_label(id),
        fields = fields,
        disabled = if playground.supported { "" } else { "disabled" },
        result_html = result_html
    )
}

#[derive(Debug, Deserialize)]
struct PrometheusResponse {
    status: String,
    data: PrometheusData,
}

#[derive(Debug, Deserialize)]
struct PrometheusData {
    result: Vec<PrometheusResult>,
}

#[derive(Debug, Deserialize)]
struct PrometheusResult {
    value: (f64, String),
}

async fn prometheus_query_scalar(client: &reqwest::Client, base: &str, query: &str) -> Option<f64> {
    let mut url = Url::parse(base).ok()?;
    url.set_path("/api/v1/query");

    let response = client
        .get(url)
        .query(&[("query", query)])
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }

    let payload: PrometheusResponse = response.json().await.ok()?;
    if payload.status != "success" {
        return None;
    }

    let value = payload.data.result.first()?.value.1.parse::<f64>().ok()?;
    Some(value)
}

fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = false;
    for ch in name.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn normalize_symbol(value: &str) -> String {
    value.rsplit("::").next().unwrap_or(value).to_string()
}

fn collect_graph_node_symbols(graph: &GraphDef) -> HashSet<String> {
    let mut symbols = HashSet::new();
    collect_graph_node_symbols_from_steps(&graph.steps, &mut symbols);
    symbols
}

fn collect_graph_node_symbols_from_steps(steps: &[GraphStep], out: &mut HashSet<String>) {
    for step in steps {
        match step {
            GraphStep::Node { name, .. } => {
                out.insert(normalize_symbol(name));
            }
            GraphStep::Nested { graph, .. } => {
                collect_graph_node_symbols_from_steps(&graph.steps, out)
            }
            GraphStep::Parallel { branches, .. } => {
                for branch in branches {
                    collect_graph_node_symbols_from_steps(branch, out);
                }
            }
            GraphStep::Route { cases, .. } => {
                for case in cases {
                    collect_graph_node_symbols_from_steps(&case.steps, out);
                }
            }
            GraphStep::While { body, .. } | GraphStep::Loop { body, .. } => {
                collect_graph_node_symbols_from_steps(body, out);
            }
            GraphStep::Break => {}
        }
    }
}

fn collect_nested_graph_defs(
    graph: &GraphDef,
    out: &mut HashMap<String, GraphDef>,
    visited: &mut HashSet<String>,
) {
    let id = slugify(graph.name);
    if !visited.insert(id) {
        return;
    }
    collect_nested_graph_defs_from_steps(&graph.steps, out, visited);
}

fn collect_nested_graph_defs_from_steps(
    steps: &[GraphStep],
    out: &mut HashMap<String, GraphDef>,
    visited: &mut HashSet<String>,
) {
    for step in steps {
        match step {
            GraphStep::Nested { graph, .. } => {
                let def = (**graph).clone();
                let id = slugify(def.name);
                out.entry(id).or_insert_with(|| def.clone());
                collect_nested_graph_defs(&def, out, visited);
            }
            GraphStep::Parallel { branches, .. } => {
                for branch in branches {
                    collect_nested_graph_defs_from_steps(branch, out, visited);
                }
            }
            GraphStep::Route { cases, .. } => {
                for case in cases {
                    collect_nested_graph_defs_from_steps(&case.steps, out, visited);
                }
            }
            GraphStep::While { body, .. } | GraphStep::Loop { body, .. } => {
                collect_nested_graph_defs_from_steps(body, out, visited);
            }
            GraphStep::Node { .. } | GraphStep::Break => {}
        }
    }
}

fn to_mermaid(graph: &GraphDef, linkable_graphs: &HashSet<String>) -> String {
    let mut lines = Vec::new();
    let mut counter = 0usize;

    // Tune layout a bit so linear graphs read cleanly and complex graphs don't
    // feel as cramped by default.
    lines.push(r#"%%{init: {"flowchart": {"curve":"basis","nodeSpacing":50,"rankSpacing":70}} }%%"#.to_string());
    // Prefer a horizontal layout so execution reads left-to-right; the UI wraps
    // the SVG in a horizontal scroller.
    lines.push("flowchart LR".to_string());

    lines.push("classDef graphRoot fill:#0b1f3a,stroke:#0b1f3a,color:#ffffff,stroke-width:2px".to_string());
    lines.push("classDef io fill:#fff7ed,stroke:#f97316,color:#7c2d12,stroke-width:2px".to_string());
    lines.push("classDef ctx fill:#eef2ff,stroke:#4f46e5,color:#1e1b4b,stroke-width:2px".to_string());
    lines.push("classDef stepNode fill:#ecfeff,stroke:#06b6d4,color:#083344,stroke-width:2px".to_string());
    lines.push("classDef stepGraph fill:#f1f5f9,stroke:#334155,color:#0f172a,stroke-width:2px,stroke-dasharray: 6 4".to_string());
    lines.push("classDef control fill:#fefce8,stroke:#eab308,color:#422006,stroke-width:2px".to_string());

    let root = next_id(&mut counter);
    lines.push(format!(r#"{root}["{}"]:::graphRoot"#, escape_label(graph.name)));

    let mut tracker = ArtifactTracker::default();

    let inputs_node = if graph.inputs.is_empty() {
        None
    } else {
        let node_id = next_id(&mut counter);
        lines.push(format!(
            r#"{node_id}(["{}"]):::io"#,
            escape_label(&format!("in: {}", graph.inputs.join(", ")))
        ));
        lines.push(format!("{root} --> {node_id}"));
        Some(node_id)
    };

    let has_ctx = graph_uses_borrowed_artifacts(graph);
    let ctx_node = if has_ctx {
        let node_id = next_id(&mut counter);
        lines.push(format!(r#"{node_id}[(ctx)]:::ctx"#));
        lines.push(format!("{root} -.-> {node_id}"));
        Some(node_id)
    } else {
        None
    };

    tracker.inputs_node = inputs_node.clone();
    tracker.ctx_node = ctx_node.clone();
    if let Some(inputs_node) = &inputs_node {
        for input in &graph.inputs {
            tracker.owned.insert(input.to_string(), inputs_node.clone());
        }
    }

    if graph.steps.is_empty() {
        return lines.join("\n");
    }

    let rendered = append_steps(
        &graph.steps,
        &mut tracker,
        linkable_graphs,
        &mut lines,
        &mut counter,
    );
    lines.push(format!("{root} --> {}", rendered.head));

    let outputs_node = if graph.outputs.is_empty() {
        None
    } else {
        let node_id = next_id(&mut counter);
        lines.push(format!(
            r#"{node_id}(["{}"]):::io"#,
            escape_label(&format!("out: {}", graph.outputs.join(", ")))
        ));
        Some(node_id)
    };

    if let Some(outputs_node) = &outputs_node {
        lines.push(format!("{} --> {outputs_node}", rendered.tail));
        // Add explicit data edges for declared graph outputs.
        for &output in graph.outputs.iter() {
            if let Some(src) = tracker.owned.get(output) {
                lines.push(format!(
                    r#"{src} -. "{}" .-> {outputs_node}"#,
                    escape_label(output)
                ));
            }
        }
    }

    lines.join("\n")
}

#[derive(Clone, Default)]
struct ArtifactTracker {
    owned: HashMap<String, String>,
    inputs_node: Option<String>,
    ctx_node: Option<String>,
}

#[derive(Clone)]
struct RenderedSteps {
    head: String,
    tail: String,
}

fn append_steps(
    steps: &[GraphStep],
    tracker: &mut ArtifactTracker,
    linkable_graphs: &HashSet<String>,
    lines: &mut Vec<String>,
    counter: &mut usize,
) -> RenderedSteps {
    let mut head: Option<String> = None;
    let mut previous_tail: Option<String> = None;

    for step in steps {
        let rendered = render_step(step, tracker, linkable_graphs, lines, counter);
        if head.is_none() {
            head = Some(rendered.head.clone());
        }
        if let Some(prev) = previous_tail {
            lines.push(format!("{prev} --> {}", rendered.head));
        }
        previous_tail = Some(rendered.tail);
    }

    RenderedSteps {
        head: head.unwrap_or_else(|| next_id(counter)),
        tail: previous_tail.unwrap_or_else(|| next_id(counter)),
    }
}

fn render_step(
    step: &GraphStep,
    tracker: &mut ArtifactTracker,
    linkable_graphs: &HashSet<String>,
    lines: &mut Vec<String>,
    counter: &mut usize,
) -> RenderedSteps {
    match step {
        GraphStep::Node {
            name,
            inputs,
            outputs,
        } => {
            let node_id = next_id(counter);
            let label = normalize_symbol(name);
            lines.push(format!(r#"{node_id}(["{}"]):::stepNode"#, escape_label(&label)));
            emit_artifact_edges(tracker, &node_id, inputs, outputs, lines);
            RenderedSteps {
                head: node_id.clone(),
                tail: node_id,
            }
        }
        GraphStep::Nested {
            graph,
            inputs,
            outputs,
        } => {
            // Keep nested graphs collapsed by default; expanding them inline makes
            // even simple graphs hard to read.
            let node_id = next_id(counter);
            lines.push(format!(
                r#"{node_id}[["{}"]]:::stepGraph"#,
                escape_label(graph.name)
            ));
            emit_artifact_edges(tracker, &node_id, inputs, outputs, lines);
            let nested_id = slugify(graph.name);
            if linkable_graphs.contains(&nested_id) {
                lines.push(format!(
                    r#"click {node_id} "/graph/{nested_id}" "Open {}" _self"#,
                    escape_label(graph.name)
                ));
                lines.push(format!(r#"style {node_id} cursor:pointer"#));
            }
            RenderedSteps {
                head: node_id.clone(),
                tail: node_id,
            }
        }
        GraphStep::Parallel {
            branches,
            inputs,
            outputs,
        } => {
            let fork = next_id(counter);
            let join = next_id(counter);
            lines.push(format!(r#"{fork}(("&")):::control"#));
            lines.push(format!(r#"{join}(("join")):::control"#));

            emit_artifact_edges(tracker, &fork, inputs, &[], lines);

            for (idx, branch) in branches.iter().enumerate() {
                if branch.is_empty() {
                    continue;
                }
                let mut branch_tracker = tracker.clone();
                let rendered =
                    append_steps(branch, &mut branch_tracker, linkable_graphs, lines, counter);
                lines.push(format!(r#"{fork} -->|b{}| {}"#, idx + 1, rendered.head));
                lines.push(format!("{} --> {join}", rendered.tail));

                for &output in outputs.iter() {
                    let (base, borrowed) = parse_artifact(output);
                    if borrowed {
                        continue;
                    }
                    if let Some(src) = branch_tracker.owned.get(base) {
                        lines.push(format!(
                            r#"{src} -. "{}" .-> {join}"#,
                            escape_label(base)
                        ));
                    }
                }
            }

            // Join outputs are the union of branch exit artifacts.
            for &output in outputs.iter() {
                let (base, borrowed) = parse_artifact(output);
                if borrowed {
                    if let Some(ctx) = &tracker.ctx_node {
                        lines.push(format!(
                            r#"{join} -. "{}" .-> {ctx}"#,
                            escape_label(output)
                        ));
                    }
                } else {
                    tracker.owned.insert(base.to_string(), join.clone());
                }
            }

            RenderedSteps { head: fork, tail: join }
        }
        GraphStep::Route {
            on,
            cases,
            inputs,
            outputs,
        } => {
            let decision = next_id(counter);
            let join = next_id(counter);

            lines.push(format!(
                r#"{decision}{{"{}"}}:::control"#,
                escape_label(&route_label(on, inputs))
            ));
            lines.push(format!(r#"{join}(("join")):::control"#));

            emit_artifact_edges(tracker, &decision, inputs, &[], lines);

            for case in cases {
                if case.steps.is_empty() {
                    continue;
                }
                let mut case_tracker = tracker.clone();
                let rendered =
                    append_steps(&case.steps, &mut case_tracker, linkable_graphs, lines, counter);
                lines.push(format!(
                    r#"{decision} -->|"{}"| {}"#,
                    escape_label(case.label),
                    rendered.head
                ));
                lines.push(format!("{} --> {join}", rendered.tail));

                for &output in outputs.iter() {
                    let (base, borrowed) = parse_artifact(output);
                    if borrowed {
                        continue;
                    }
                    if let Some(src) = case_tracker.owned.get(base) {
                        lines.push(format!(
                            r#"{src} -. "{}" .-> {join}"#,
                            escape_label(base)
                        ));
                    }
                }
            }

            for &output in outputs.iter() {
                let (base, borrowed) = parse_artifact(output);
                if borrowed {
                    if let Some(ctx) = &tracker.ctx_node {
                        lines.push(format!(
                            r#"{join} -. "{}" .-> {ctx}"#,
                            escape_label(output)
                        ));
                    }
                } else {
                    tracker.owned.insert(base.to_string(), join.clone());
                }
            }

            RenderedSteps {
                head: decision,
                tail: join,
            }
        }
        GraphStep::While {
            condition,
            body,
            inputs,
            outputs,
        } => {
            let cond = next_id(counter);
            let exit = next_id(counter);
            lines.push(format!(
                r#"{cond}{{"{}"}}:::control"#,
                escape_label(&format!("while {condition}"))
            ));
            lines.push(format!(r#"{exit}(("exit")):::control"#));

            emit_artifact_edges(tracker, &cond, inputs, &[], lines);

            if !body.is_empty() {
                let mut body_tracker = tracker.clone();
                let rendered =
                    append_steps(body, &mut body_tracker, linkable_graphs, lines, counter);
                lines.push(format!(r#"{cond} -->|"true"| {}"#, rendered.head));
                lines.push(format!("{} --> {cond}", rendered.tail));
            }
            lines.push(format!(r#"{cond} -->|"false"| {exit}"#));

            for &output in outputs.iter() {
                let (base, borrowed) = parse_artifact(output);
                if borrowed {
                    if let Some(ctx) = &tracker.ctx_node {
                        lines.push(format!(
                            r#"{exit} -. "{}" .-> {ctx}"#,
                            escape_label(output)
                        ));
                    }
                } else {
                    tracker.owned.insert(base.to_string(), exit.clone());
                }
            }

            RenderedSteps { head: cond, tail: exit }
        }
        GraphStep::Loop {
            body,
            inputs,
            outputs,
        } => {
            let start = next_id(counter);
            let exit = next_id(counter);
            lines.push(format!(r#"{start}(("loop")):::control"#));
            lines.push(format!(r#"{exit}(("exit")):::control"#));
            lines.push(format!(r#"{start} -->|"exit"| {exit}"#));

            emit_artifact_edges(tracker, &start, inputs, &[], lines);

            if !body.is_empty() {
                let mut body_tracker = tracker.clone();
                let rendered =
                    append_steps(body, &mut body_tracker, linkable_graphs, lines, counter);
                lines.push(format!("{start} --> {}", rendered.head));
                lines.push(format!("{} --> {start}", rendered.tail));
            }

            // The macro's `Break` is modeled as a step; leave the explicit break
            // node to visually indicate exits.
            for &output in outputs.iter() {
                let (base, borrowed) = parse_artifact(output);
                if borrowed {
                    if let Some(ctx) = &tracker.ctx_node {
                        lines.push(format!(
                            r#"{exit} -. "{}" .-> {ctx}"#,
                            escape_label(output)
                        ));
                    }
                } else {
                    tracker.owned.insert(base.to_string(), exit.clone());
                }
            }

            RenderedSteps { head: start, tail: exit }
        }
        GraphStep::Break => {
            let node_id = next_id(counter);
            lines.push(format!(r#"{node_id}(("break")):::control"#));
            RenderedSteps {
                head: node_id.clone(),
                tail: node_id,
            }
        }
    }
}

fn graph_uses_borrowed_artifacts(graph: &GraphDef) -> bool {
    steps_use_borrows(&graph.steps)
}

fn steps_use_borrows(steps: &[GraphStep]) -> bool {
    for step in steps {
        match step {
            GraphStep::Node {
                inputs, outputs, ..
            } => {
                if inputs.iter().any(|v| v.starts_with('&')) || outputs.iter().any(|v| v.starts_with('&')) {
                    return true;
                }
            }
            GraphStep::Nested {
                inputs, outputs, ..
            } => {
                if inputs.iter().any(|v| v.starts_with('&')) || outputs.iter().any(|v| v.starts_with('&')) {
                    return true;
                }
            }
            GraphStep::Parallel { branches, .. } => {
                if branches.iter().any(|b| steps_use_borrows(b)) {
                    return true;
                }
            }
            GraphStep::Route { cases, .. } => {
                if cases.iter().any(|c| steps_use_borrows(&c.steps)) {
                    return true;
                }
            }
            GraphStep::While { body, .. } | GraphStep::Loop { body, .. } => {
                if steps_use_borrows(body) {
                    return true;
                }
            }
            GraphStep::Break => {}
        }
    }
    false
}

fn parse_artifact(value: &str) -> (&str, bool) {
    if let Some(rest) = value.strip_prefix('&') {
        (rest, true)
    } else {
        (value, false)
    }
}

fn route_label(on: &str, inputs: &[&'static str]) -> String {
    if inputs.len() == 1 {
        return format!("match {}", inputs[0]);
    }
    let trimmed = on.trim();
    let noisy = trimmed.contains('{') || trimmed.contains('|') || trimmed.len() > 60;
    if noisy {
        "match".to_string()
    } else {
        format!("match {trimmed}")
    }
}

fn emit_artifact_edges(
    tracker: &mut ArtifactTracker,
    step_node: &str,
    inputs: &[&'static str],
    outputs: &[&'static str],
    lines: &mut Vec<String>,
) {
    for input in inputs {
        let (base, borrowed) = parse_artifact(input);
        if borrowed {
            if let Some(ctx) = &tracker.ctx_node {
                lines.push(format!(r#"{ctx} -. "{}" .-> {step_node}"#, escape_label(input)));
            }
            continue;
        }
        if let Some(src) = tracker.owned.get(base) {
            lines.push(format!(
                r#"{src} -. "{}" .-> {step_node}"#,
                escape_label(base)
            ));
        } else if let Some(inputs_node) = &tracker.inputs_node {
            lines.push(format!(
                r#"{inputs_node} -. "{}" .-> {step_node}"#,
                escape_label(base)
            ));
            tracker.owned.insert(base.to_string(), inputs_node.clone());
        }
    }

    for output in outputs {
        let (base, borrowed) = parse_artifact(output);
        if borrowed {
            if let Some(ctx) = &tracker.ctx_node {
                lines.push(format!(r#"{step_node} -. "{}" .-> {ctx}"#, escape_label(output)));
            }
            continue;
        }
        tracker.owned.insert(base.to_string(), step_node.to_string());
    }
}

fn escape_label(value: &str) -> String {
    value.replace('"', "'").replace('\n', " ")
}

fn next_id(counter: &mut usize) -> String {
    let id = format!("n{counter}");
    *counter += 1;
    id
}

#[derive(Debug)]
struct AppHttpError {
    code: StatusCode,
    message: String,
}

impl AppHttpError {
    fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
}

impl IntoResponse for AppHttpError {
    fn into_response(self) -> Response {
        (self.code, self.message).into_response()
    }
}
