
use futures::executor::block_on;
use graphium_macro::{graph, node};
use graphium;

#[test]
fn e2e_graph_async_with_sync_nodes() {
    #[derive(Default)]
    struct Context {
        pub number: u32
    }

    let mut ctx = Context::default();

    

    node! {
        fn set_ctx(ctx: &mut Context) {
            ctx.number = 5;
        }
    }

    graph! {
        #[metadata(context = Context, async = true)]
        AsyncSyncGraph {
            SetCtx()
        }
    }

    block_on(AsyncSyncGraph::__graphium_run_async(&mut ctx));
    assert_eq!(ctx.number, 5);
}

#[test]
fn e2e_graph_async_nodes() {
    let mut ctx = graphium::Context::default();

    node! {
        async fn get_number() -> u32 {
            7
        }
    }

    node! {
        async fn add_one(value: u32) -> u32 {
            value + 1
        }
    }

    graph! {
        #[metadata(context = graphium::Context, outputs = (a_number: u32), async = true)]
        AsyncGraph {
            GetNumber() -> (a_number) >>
            AddOne(a_number) -> (a_number)
        }
    }

    let value = block_on(AsyncGraph::__graphium_run_async(&mut ctx));
    assert_eq!(value, 8);
}
