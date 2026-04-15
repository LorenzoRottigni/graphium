# graphium-ui

`graphium-ui` is a lightweight web app for visualizing Graphium graphs and their Prometheus metrics.

## What it does

- Takes a configured list of Graphium graph types (`GraphDefProvider`)
- Connects to a Prometheus HTTP API
- Home page forces graph selection
- Shows graph visual representation + graph metrics after selection

## Quick usage

```rust
use graphium_macro::{graph, node};
use graphium_ui::{graphs, GraphiumUiConfig};

#[derive(Default)]
struct Context;

node! {
    fn seed() -> u32 { 1 }
}

graph! {
    #[metadata(context = Context)]
    DemoGraph {
        Seed()
    }
}

#[tokio::main]
async fn main() {
    let config = GraphiumUiConfig::from_graphs(
        "http://127.0.0.1:9090",
        graphs![DemoGraph],
    );

    graphium_ui::serve(config).await.unwrap();
}
```

You can also build config from a `config.rs` module:

```rust
use graphium_ui::{graphs, GraphiumUiConfig};

pub fn config() -> GraphiumUiConfig {
    GraphiumUiConfig {
        prometheus_url: "http://127.0.0.1:9000".into(),
        graphs: graphs![OwnedGraph, BorrowedGraph],
        ..Default::default()
    }
}
```
