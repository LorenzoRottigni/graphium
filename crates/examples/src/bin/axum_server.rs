use std::net::SocketAddr;

use axum::{Router, response::IntoResponse, routing::get};
use graphium_macro::{graph, node};

#[derive(Clone)]
struct Context {
    router: Router,
    addr: SocketAddr,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            router: Router::new(),
            addr: "127.0.0.1:3000".parse().expect("valid address"),
        }
    }
}

async fn root_handler() -> &'static str {
    "graphium axum server"
}

async fn health_handler() -> &'static str {
    "ok"
}

async fn ping_handler() -> &'static str {
    "pong"
}

async fn version_handler() -> &'static str {
    "v0"
}

async fn metrics_handler() -> impl IntoResponse {
    graphium::metrics::render_prometheus()
}

node! {
    fn init_router(ctx: &mut Context) {
        ctx.router = Router::new();
    }
}

node! {
    fn add_root_route(ctx: &mut Context) {
        let router = std::mem::take(&mut ctx.router);
        ctx.router = router.route("/", get(root_handler));
    }
}

node! {
    fn add_health_route(ctx: &mut Context) {
        let router = std::mem::take(&mut ctx.router);
        ctx.router = router.route("/health", get(health_handler));
    }
}

node! {
    fn add_ping_route(ctx: &mut Context) {
        let router = std::mem::take(&mut ctx.router);
        ctx.router = router.route("/ping", get(ping_handler));
    }
}

node! {
    fn add_version_route(ctx: &mut Context) {
        let router = std::mem::take(&mut ctx.router);
        ctx.router = router.route("/version", get(version_handler));
    }
}

node! {
    fn add_metrics_route(ctx: &mut Context) {
        let router = std::mem::take(&mut ctx.router);
        ctx.router = router.route("/metrics", get(metrics_handler));
    }
}

node! {
    async fn start_server(ctx: &mut Context) {
        let listener = tokio::net::TcpListener::bind(ctx.addr)
            .await
            .expect("bind address");
        axum::serve(listener, ctx.router.clone())
            .await
            .expect("serve failed");
    }
}

graph! {
    async AxumServerGraph<Context> {
        InitRouter() >>
        AddRootRoute() >>
        AddHealthRoute() >>
        AddPingRoute() >>
        AddVersionRoute() >>
        AddMetricsRoute() >>
        StartServer()
    }
}

#[tokio::main]
async fn main() {
    let mut ctx = Context::default();
    AxumServerGraph::run_async(&mut ctx).await;
}
