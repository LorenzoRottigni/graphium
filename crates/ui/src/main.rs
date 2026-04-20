use std::net::SocketAddr;

use graphium_ui::GraphiumUiConfig;

#[tokio::main]
async fn main() {
    let bind: SocketAddr = std::env::var("GRAPHIUM_UI_BIND")
        .unwrap_or_else(|_| "127.0.0.1:4001".to_string())
        .parse()
        .expect("valid GRAPHIUM_UI_BIND socket address");
    let prometheus = std::env::var("GRAPHIUM_PROMETHEUS_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:9090".to_string());

    let config = GraphiumUiConfig {
        bind,
        prometheus_url: prometheus,
        ..Default::default()
    };
    if let Err(err) = graphium_ui::serve(config).await {
        eprintln!("graphium-ui failed: {err}");
        std::process::exit(1);
    }
}
