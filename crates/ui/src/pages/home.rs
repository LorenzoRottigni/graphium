use std::fmt::Write as _;

use crate::state::AppState;
use crate::util::escape_label;

pub(crate) fn home_page_html(state: &AppState) -> String {
    let mut options = String::new();
    for graph in &state.ordered {
        let _ = writeln!(
            options,
            r#"<option value="{}">{}</option>"#,
            escape_label(&graph.id),
            escape_label(&graph.name)
        );
    }

    let body = format!(
        r#"<section class="home-hero">
  <div class="home-logo" aria-hidden="true">
    {logo}
  </div>
  <h1 class="home-title">Graphium UI</h1>
  <p class="home-tagline">
    Visualize your <strong>Graphium</strong> graphs, inspect Prometheus metrics, and run playgrounds/tests from a single dashboard.
  </p>

  <section class="card home-cta">
    <h2 style="margin:0 0 .35rem 0;">Open a dashboard</h2>
    <p class="muted" style="margin:.1rem 0 1rem 0;">
      Pick a graph and jump straight into its structure + metrics view.
    </p>
    <form method="get" action="/select" class="home-form">
      <select name="id" aria-label="Select graph">
        {options}
      </select>
      <button type="submit">Start dashboard</button>
    </form>
    <div class="home-links muted">
      <a href="/dashboard">Dashboard</a>
      <span>·</span>
      <a href="/tests">Tests</a>
    </div>
  </section>
</section>"#,
        options = options,
        logo = logo_svg()
    );

    format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Graphium UI</title>
  <style>{css}</style>
</head>
<body>
  <div class="container">
    {body}
  </div>
</body>
</html>"#,
        css = HOME_CSS,
        body = body
    )
}

fn logo_svg() -> &'static str {
    r###"<img src="https://s3.rottigni.tech/public/github/graphium_logo.png" alt="graphium" width="250px" />"###
}

const HOME_CSS: &str = r#"
  :root {
    --bg: #0b0b0c;
    --fg: #e5e7eb;
    --card: #121214;
    --muted: #9ca3af;
    --border: #2a2a2f;
    --primary: #f97316;
    --shadow: rgba(0, 0, 0, 0.35);
  }

  body {
    font-family: ui-sans-serif, system-ui, -apple-system, sans-serif;
    margin: 0;
    background: radial-gradient(1200px 600px at 50% 0%, rgba(249,115,22,0.14), transparent 55%),
      var(--bg);
    color: var(--fg);
  }

  .container {
    max-width: 1100px;
    margin: 0 auto;
    padding: 1.2rem;
  }

  .muted { color: var(--muted); }

  select, button {
    padding: .75rem .9rem;
    font-size: 1rem;
    border-radius: 12px;
    border: 1px solid #3a3a42;
    background: #0f0f12;
    color: var(--fg);
  }

  button {
    background: var(--primary);
    border: none;
    cursor: pointer;
    font-weight: 700;
  }

  .card {
    background: var(--card);
    border-radius: 16px;
    box-shadow: 0 14px 30px var(--shadow);
    padding: 1.25rem;
    border: 1px solid rgba(249,115,22,0.18);
  }

  .home-hero {
    min-height: calc(100vh - 2.4rem);
    display: grid;
    place-items: center;
    text-align: center;
    padding: 2.2rem 0;
  }

  .home-logo { display: grid; place-items: center; margin-bottom: .75rem; }
  .home-title { margin: .2rem 0; font-size: 2.35rem; letter-spacing: .01em; }

  .home-tagline {
    max-width: 760px;
    margin: .25rem auto 1.35rem auto;
    color: var(--muted);
    font-size: 1.08rem;
    line-height: 1.6;
  }

  .home-cta { width: min(820px, 100%); }

  .home-form {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: .85rem;
    align-items: center;
    margin-top: .2rem;
  }

  .home-links {
    margin-top: .9rem;
    display: flex;
    gap: .6rem;
    justify-content: center;
    align-items: center;
  }

  .home-links a {
    color: var(--muted);
    text-decoration: none;
    font-weight: 700;
  }

  .home-links a:hover { color: var(--fg); }

  @media (max-width: 720px) {
    .home-form { grid-template-columns: 1fr; }
    .home-title { font-size: 2.0rem; }
  }
"#;
