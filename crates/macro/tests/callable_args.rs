use graphium_macro::{graph, node};

#[derive(Default)]
struct TestContext;

node! {
    fn get_product(ctx: &TestContext) -> u32 {
        let _ = ctx;
        10
    }
}

node! {
    fn load_handler(
        ctx: &TestContext,
        handler: GetProduct,
        server: u32,
    ) -> u32 {
        ::graphium::NodeHandle::run(&handler, ctx, ()) + server
    }
}

node! {
    fn server_value() -> u32 {
        5
    }
}

graph! {
    ExampleGraph<TestContext> -> (value: u32) {
        ServerValue() -> (server) >>
        LoadHandler(GetProduct, server) -> (value)
    }
}

node! {
    fn run_graph(
        ctx: &mut TestContext,
        runner: InnerGraph,
    ) -> u32 {
        ::graphium::GraphHandle::run(&runner, ctx, ())
    }
}

graph! {
    InnerGraph<TestContext> -> (value: u32) {
        GetProduct() -> (value)
    }
}

graph! {
    OuterGraph<TestContext> -> (value: u32) {
        RunGraph(InnerGraph) -> (value)
    }
}

#[test]
fn e2e_node_callable_argument_executes_inner_node() {
    let mut ctx = TestContext::default();
    let value = ExampleGraph::run(&mut ctx);
    assert_eq!(value, 15);
}

#[test]
fn e2e_graph_callable_argument_executes_inner_graph() {
    let mut ctx = TestContext::default();
    let value = OuterGraph::run(&mut ctx);
    assert_eq!(value, 10);
}
