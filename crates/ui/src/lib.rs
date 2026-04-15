use std::collections::HashMap;
use std::fmt::Write as _;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use graphium::{GraphDef, GraphDefProvider, GraphStep};
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

    pub fn with_graph<G: GraphDefProvider + 'static>(mut self) -> Self {
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
}

impl ConfiguredGraph {
    pub fn from_graph_def(def: GraphDef) -> Self {
        let id = slugify(def.name);
        Self {
            id,
            name: def.name.to_string(),
            def,
        }
    }

    pub fn from_provider<G: GraphDefProvider + 'static>() -> Self {
        Self::from_graph_def(G::graph_def())
    }
}

pub fn graph<G: GraphDefProvider + 'static>() -> ConfiguredGraph {
    ConfiguredGraph::from_graph_def(G::graph_def())
}

#[macro_export]
macro_rules! graphs {
    ($($graph:path),+ $(,)?) => {{
        vec![
            $(
                $crate::ConfiguredGraph::from_graph_def(
                    <$graph as ::graphium::GraphDefProvider>::graph_def()
                )
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

    let by_id = config
        .graphs
        .iter()
        .cloned()
        .map(|g| (g.id.clone(), g))
        .collect::<HashMap<_, _>>();

    let state = Arc::new(AppState {
        prometheus_base_url: config.prometheus_url,
        client: reqwest::Client::new(),
        ordered: config.graphs,
        by_id,
    });

    let app = Router::new()
        .route("/", get(home))
        .route("/select/:id", get(select_graph))
        .route("/graph/:id", get(graph_page))
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
    let graph = state
        .by_id
        .get(&id)
        .ok_or_else(|| AppHttpError::not_found("graph not configured"))?;

    let mermaid = to_mermaid(&graph.def);
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
    .layout {{ display: grid; grid-template-columns: 1.8fr 1fr; gap: 1rem; }}
    .card {{ background: white; border-radius: 14px; box-shadow: 0 10px 20px rgba(0,0,0,.06); padding: 1rem; overflow: auto; }}
    .metrics {{ display: grid; grid-template-columns: 1fr 1fr; gap: .75rem; }}
    .metric {{ border: 1px solid #ebeff5; border-radius: 12px; padding: .65rem; }}
    .metric .k {{ font-size: .8rem; opacity: .75; }}
    .metric .v {{ font-size: 1rem; font-weight: 700; margin-top: .2rem; }}
    @media (max-width: 960px) {{ .layout {{ grid-template-columns: 1fr; }} }}
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
      <a href="/" style="margin-left:.4rem;">Home</a>
    </form>
  </header>

  <section class="layout">
    <article class="card">
      <h3>Graph structure</h3>
      <pre class="mermaid">{mermaid}</pre>
    </article>
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

fn to_mermaid(graph: &GraphDef) -> String {
    let mut lines = vec!["flowchart TD".to_string()];
    let mut counter = 0usize;

    let root = next_id(&mut counter);
    lines.push(format!(r#"{root}["{}"]"#, escape_label(graph.name)));
    append_steps(&graph.steps, &root, &mut lines, &mut counter);
    lines.join("\n")
}

fn append_steps(steps: &[GraphStep], parent: &str, lines: &mut Vec<String>, counter: &mut usize) {
    let mut previous = parent.to_string();

    for step in steps {
        let node_id = next_id(counter);
        let label = match step {
            GraphStep::Node { name, .. } => format!("Node: {name}"),
            GraphStep::Nested { graph, .. } => format!("Nested: {}", graph.name),
            GraphStep::Parallel { .. } => "Parallel (&)".to_string(),
            GraphStep::Route { on, .. } => format!("Route: {on}"),
            GraphStep::While { condition, .. } => format!("While: {condition}"),
            GraphStep::Loop { .. } => "Loop".to_string(),
            GraphStep::Break => "Break".to_string(),
        };

        lines.push(format!(r#"{node_id}["{}"]"#, escape_label(&label)));
        lines.push(format!("{previous} --> {node_id}"));

        match step {
            GraphStep::Nested { graph, .. } => append_steps(&graph.steps, &node_id, lines, counter),
            GraphStep::Parallel { branches } => {
                for (idx, branch) in branches.iter().enumerate() {
                    let branch_root = next_id(counter);
                    lines.push(format!(r#"{branch_root}["Branch {}"]"#, idx + 1));
                    lines.push(format!("{node_id} --> {branch_root}"));
                    append_steps(branch, &branch_root, lines, counter);
                }
            }
            GraphStep::Route { cases, .. } => {
                for case in cases {
                    let case_root = next_id(counter);
                    lines.push(format!(
                        r#"{case_root}["Case {}"]"#,
                        escape_label(case.label)
                    ));
                    lines.push(format!("{node_id} --> {case_root}"));
                    append_steps(&case.steps, &case_root, lines, counter);
                }
            }
            GraphStep::While { body, .. } | GraphStep::Loop { body } => {
                append_steps(body, &node_id, lines, counter)
            }
            GraphStep::Node { .. } | GraphStep::Break => {}
        }

        previous = node_id;
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
