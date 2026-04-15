use graphium_macro::{graph, node};
use graphium_ui::{GraphiumUiConfig, graphs};

#[derive(Default)]
struct Context {
    a_number: u32,
}

node! {
    fn get_number() -> u32 {
        42
    }
}

node! {
    fn duplicate(value: u32) -> (u32, u32) {
        (value, value)
    }
}

node! {
    fn pipe_number(value: u32) -> u32 {
        value
    }
}

node! {
    fn store_number(ctx: &mut Context, a_number: u32) {
        ctx.a_number = a_number;
    }
}

node! {
    fn take_ownership(a_number: &u32) -> u32 {
        *a_number
    }
}

graph! {
    #[metadata(context = Context, outputs = (a_split: u32))]
    OwnedGraph {
        GetNumber() -> (a_number) >>
        Duplicate(a_number) -> (a_split, b_split) >>
        PipeNumber(a_split) -> (a_split)
    }
}

graph! {
    #[metadata(context = Context, outputs = (a_number: u32))]
    BorrowedGraph {
        GetNumber() -> (a_number) >>
        StoreNumber(a_number) -> (&a_number) >>
        TakeOwnership(&a_number) -> (a_number) >>
        PipeNumber(a_number) -> (a_number)
    }
}

pub fn config() -> GraphiumUiConfig {
    GraphiumUiConfig {
        prometheus_url: std::env::var("GRAPHIUM_PROMETHEUS_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9090".to_string()),
        graphs: graphs![OwnedGraph, BorrowedGraph],
        ..Default::default()
    }
}
