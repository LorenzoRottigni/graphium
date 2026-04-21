use axum::Router;

pub mod context;
pub mod nodes;
pub mod graphs;
pub mod routes;

#[tokio::main]
pub async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("bind address");
    let router = Router::new().route("/product/create", routes::create_product());

    axum::serve(listener, router)
        .await
        .expect("serve failed");
}
