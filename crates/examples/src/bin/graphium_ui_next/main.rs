mod config;

#[tokio::main]
async fn main() {
    let config = config::config();
    if let Err(err) = graphium_ui_next::server::serve(config).await {
        eprintln!("graphium-ui-next example failed: {err}");
        std::process::exit(1);
    }
}
