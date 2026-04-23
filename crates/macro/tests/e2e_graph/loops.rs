use graphium_macro::{graph, node};

#[test]
fn e2e_graph_while_loop() {
    #[derive(Default)]
    pub struct Context {
        pub number: u32,
    }

    let mut ctx = Context::default();

    node! {
        fn init_ctx(ctx: &mut Context) {
            ctx.number = 0;
        }
    }

    node! {
        fn inc_ctx(ctx: &mut Context) {
            ctx.number += 1;
        }
    }

    graph! {
        WhileGraph<Context> {
            InitCtx() >>
            @while |ctx: &Context| ctx.number < 3 {
                IncCtx()
            }
        }
    }

    WhileGraph::run(&mut ctx);
    assert_eq!(ctx.number, 3);
}

#[test]
fn e2e_graph_loop_with_break() {
    #[derive(Default)]
    pub struct Context {
        pub number: u32,
    }

    let mut ctx = Context::default();

    node! {
        fn init_ctx(ctx: &mut Context) {
            ctx.number = 0;
        }
    }

    node! {
        fn inc_ctx(ctx: &mut Context) {
            ctx.number += 1;
        }
    }

    node! {
        fn noop() {}
    }

    graph! {
        LoopBreakGraph<Context> {
            InitCtx() >>
            @loop {
                IncCtx() >>
                @if |ctx: &Context| ctx.number >= 3 {
                    @break
                }
                @else {
                    Noop()
                }
            }
        }
    }

    LoopBreakGraph::run(&mut ctx);
    assert_eq!(ctx.number, 3);
}
