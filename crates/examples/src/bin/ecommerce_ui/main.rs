#[path = "../ecommerce/context.rs"]
pub mod context;
#[path = "../ecommerce/graphs/mod.rs"]
pub mod graphs;
#[path = "../ecommerce/models.rs"]
pub mod models;
#[path = "../ecommerce/nodes/mod.rs"]
pub mod nodes;

mod config;

#[tokio::main]
async fn main() {
    let config = config::config();
    if let Err(err) = graphium_ui::server::serve(config).await {
        eprintln!("ecommerce graphium-ui failed: {err}");
        std::process::exit(1);
    }
}
