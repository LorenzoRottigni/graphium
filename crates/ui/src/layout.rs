use crate::util::escape_label;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveNav {
    Graphs,
    Tests,
}

pub(crate) struct LayoutContext {
    pub(crate) active: ActiveNav,
    pub(crate) header_extra_html: Option<String>,
    pub(crate) alpine_data: Option<String>,
    pub(crate) extra_head_html: Option<String>,
}

impl LayoutContext {
    pub(crate) fn graphs(_state: &crate::state::AppState, selected_graph_id: &str) -> Self {
        Self {
            active: ActiveNav::Graphs,
            header_extra_html: None,
            alpine_data: Some(format!(
                "{{ graphId: '{}' }}",
                escape_label(selected_graph_id)
            )),
            extra_head_html: None,
        }
    }

    pub(crate) fn tests(_state: &crate::state::AppState) -> Self {
        Self {
            active: ActiveNav::Tests,
            header_extra_html: None,
            alpine_data: None,
            extra_head_html: None,
        }
    }

    pub(crate) fn set_header_extra_html(mut self, html: String) -> Self {
        self.header_extra_html = Some(html);
        self
    }

    pub(crate) fn set_extra_head_html(mut self, html: String) -> Self {
        self.extra_head_html = Some(html);
        self
    }
}

pub(crate) fn render_page(title: &str, ctx: LayoutContext, main_html: String) -> String {
    let header = header_html(&ctx);
    let footer = footer_html();
    let alpine = ctx
        .alpine_data
        .as_deref()
        .map(|v| format!(r#" x-data="{v}""#))
        .unwrap_or_default();
    let extra_head = ctx.extra_head_html.unwrap_or_default();

    format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title}</title>
  <style>{css}</style>
  <script src="https://unpkg.com/htmx.org@1.9.12"></script>
  <script defer src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js"></script>
  {extra_head}
</head>
<body{alpine}>
  <div class="container">
    {header}
    <main class="main">
      {main_html}
    </main>
    {footer}
  </div>
</body>
</html>"#,
        title = escape_label(title),
        css = BASE_CSS,
        header = header,
        footer = footer,
        main_html = main_html,
        alpine = alpine,
        extra_head = extra_head
    )
}

fn header_html(ctx: &LayoutContext) -> String {
    let active_graphs = if ctx.active == ActiveNav::Graphs {
        "nav-active"
    } else {
        ""
    };
    let active_tests = if ctx.active == ActiveNav::Tests {
        "nav-active"
    } else {
        ""
    };

    let nav = format!(
        r#"<nav class="nav">
  <a class="{active_graphs}" href="/">Graphs</a>
  <a class="{active_tests}" href="/tests">Tests</a>
</nav>"#,
        active_graphs = active_graphs,
        active_tests = active_tests
    );

    let extra = ctx.header_extra_html.clone().unwrap_or_default();
    format!(
        r#"<header class="header">
  <div class="brand">
    <div class="brand-title"><img src="https://s3.rottigni.tech/public/github/graphium_logo.png" alt="graphium" width="175px" /></div>
    {nav}
  </div>
  <div class="header-right">
    {extra}
    <div class="loading" id="loading">Loading…</div>
  </div>
</header>"#,
        nav = nav,
        extra = extra
    )
}

fn footer_html() -> String {
    r#"<footer class="footer">
  <span class="muted">Graphium UI</span>
</footer>"#
        .to_string()
}

const BASE_CSS: &str = r#"
  :root {
    --bg: #f7f9fc;
    --fg: #1c2733;
    --card: #ffffff;
    --muted: #5f7388;
    --border: #e6ebf2;
    --blue: #0f7bff;
  }

  body {
    font-family: ui-sans-serif, system-ui, -apple-system, sans-serif;
    margin: 0;
    background: var(--bg);
    color: var(--fg);
  }

  .container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 1.2rem;
  }

  .header {
    display: flex;
    gap: 1rem;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    margin-bottom: 1rem;
  }

  .brand {
    display: flex;
    gap: 1rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .brand-title {
    font-size: 1.35rem;
    font-weight: 800;
    letter-spacing: .01em;
  }

  .nav a {
    text-decoration: none;
    color: var(--muted);
    margin-right: .75rem;
    font-weight: 600;
  }

  .nav a.nav-active {
    color: var(--fg);
  }

  .header-right {
    display: flex;
    gap: .8rem;
    align-items: center;
    flex-wrap: wrap;
  }

  select, button, input[type="text"] {
    padding: .6rem .8rem;
    font-size: .95rem;
    border-radius: 10px;
    border: 1px solid #d5dce5;
  }

  button {
    background: var(--blue);
    color: white;
    border: none;
    cursor: pointer;
  }

  .loading { display:none; opacity:.7; }
  .htmx-request .loading { display:inline; }

  .footer {
    margin-top: 1.2rem;
    padding-top: 1.1rem;
    border-top: 1px solid var(--border);
    opacity: .85;
  }

  .muted { color: var(--muted); }

  .card {
    background: var(--card);
    border-radius: 14px;
    box-shadow: 0 10px 20px rgba(0,0,0,.06);
    padding: 1rem;
    overflow: auto;
  }

  .hero { min-height: 420px; }

  .below {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
    margin-top: 1rem;
    align-items: start;
  }

  .side-stack {
    display:grid;
    grid-template-columns: 1fr;
    gap: 1rem;
  }

  .mermaid-scroll { overflow-x: auto; overflow-y: auto; padding-bottom: .35rem; }
  .mermaid-scroll svg { max-width: none !important; }
  pre.mermaid { margin: 0; }

  .metrics { display: grid; grid-template-columns: 1fr 1fr; gap: .75rem; }
  .metric { border: 1px solid #ebeff5; border-radius: 12px; padding: .65rem; }
  .metric .k { font-size: .8rem; opacity: .75; }
  .metric .v { font-size: 1rem; font-weight: 700; margin-top: .2rem; }

  .tests-stack { display:grid; grid-template-columns: 1fr; gap: 1rem; }
  .test-item { border: 1px solid #ebeff5; border-radius: 10px; padding: .55rem; display:flex; align-items:center; gap:.5rem; }
  .test-target { font-size: .83rem; color: var(--muted); }
  .test-name { font-size: .9rem; font-weight: 600; flex:1; }
  .test-run { text-decoration: none; background: var(--blue); color: white; border-radius: 8px; padding: .3rem .55rem; font-size: .84rem; }

  .play-label { font-size: .84rem; opacity: .8; margin-top: .3rem; }
  .play-field { display: grid; grid-template-columns: 1fr; gap: .4rem; margin: .55rem 0; }
  .play-out { background: #f2f6fb; border-radius: 10px; padding: .75rem; overflow: auto; }

  .test-card { background: white; border-radius: 12px; padding: 1rem; border: 1px solid var(--border); display: flex; align-items: center; gap: .8rem; flex-wrap: wrap; }
  .test-card .kind { font-size: .78rem; text-transform: uppercase; letter-spacing: .04em; color: #47617a; background: #edf4fb; padding: .25rem .5rem; border-radius: 999px; }
  .test-card .name { font-weight: 600; flex: 1; }
  .test-card .target { font-size:.86rem; color: var(--muted); flex-basis: 100%; }
  .test-card .run { text-decoration: none; background: var(--blue); color: white; border-radius: 8px; padding: .45rem .7rem; }

  @media (max-width: 960px) { .below { grid-template-columns: 1fr; } }
"#;
