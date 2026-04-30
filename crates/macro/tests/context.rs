use graphium_macro::{graph, node};

#[test]
fn e2e_graph_macro_wires_ctx() {
    #[derive(Default)]
    pub struct Context {
        pub number: u32,
    }

    let mut ctx = Context::default();

    node! {
        fn get_mutable_ctx(ctx: &mut Context) {
            ctx.number = 42;
        }
    }

    node! {
        fn extract_from_ctx(ctx: &Context) -> u32 {
            ctx.number
        }
    }

    node! {
        fn assert_ctx(number: u32, ctx: &Context) {
            assert_eq!(number, ctx.number)
        }
    }

    node! {
        fn assert_ctx_2(ctx: &Context, number: u32) {
            assert_eq!(number, ctx.number)
        }
    }

    graph! {
        CtxGraph<'a, Context> {
            GetMutableCtx() >>
            ExtractFromCtx() -> (number) >>
            AssertCtx(number) && AssertCtx2(number)
        }
    }

    CtxGraph::run(&mut ctx);
}
