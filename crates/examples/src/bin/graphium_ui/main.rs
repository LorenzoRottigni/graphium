mod config;

#[tokio::main]
async fn main() {
    let config = config::config();
    if let Err(err) = graphium_ui::serve(config).await {
        eprintln!("graphium-ui example failed: {err}");
        std::process::exit(1);
    }
}
