use graphium;
use graphium_macro::{graph, node};
use futures::executor::block_on;

node! {
    fn get_number() -> u32 {
        42
    }
}

node! {
    fn pipe_number(a: u32) -> u32 {
        a
    }
}

#[test]
fn e2e_graph_macro_moves_artifacts() {
    let mut ctx = graphium::Context::default();

    node! {
        /// Duplicates a number into two outputs.
        #[tags("math", "util")]
        #[deprecated = "use `BetterDuplicate` instead"]
        fn duplicate(a: u32) -> (u32, u32) {
            (a, a)
        }
    }

    graph! {
        /// Duplicates a single number and pipes it through the graph.
        #[tags("demo", "math")]
        #[deprecated(note = "use `BetterGraph` instead")]
        OwnedGraph -> (a_split: u32) {
            GetNumber() -> (number) >>
            Duplicate(number) -> (a_split, b_split) >>
            PipeNumber(a_split) -> (a_split)
        }
    }
    let duplicated = OwnedGraph::__graphium_run(&mut ctx);
    let graph_dto = OwnedGraph::__graphium_dto();
    assert_eq!(
        graph_dto.docs.as_deref(),
        Some("Duplicates a single number and pipes it through the graph.")
    );
    assert_eq!(graph_dto.tags, vec!["demo".to_string(), "math".to_string()]);
    assert!(graph_dto.deprecated);
    assert_eq!(
        graph_dto.deprecated_reason.as_deref(),
        Some("use `BetterGraph` instead")
    );
    let node_dto = Duplicate::__graphium_dto();
    assert_eq!(
        node_dto.docs.as_deref(),
        Some("Duplicates a number into two outputs.")
    );
    assert_eq!(node_dto.tags, vec!["math".to_string(), "util".to_string()]);
    assert!(node_dto.deprecated);
    assert_eq!(
        node_dto.deprecated_reason.as_deref(),
        Some("use `BetterDuplicate` instead")
    );

    assert_eq!(duplicated, 42);
}

#[test]
fn e2e_node_macro_supports_explicit_name_override() {
    let mut ctx = graphium::Context::default();

    node! {
        #[name = getNumber]
        #[tags("io")]
        async fn get_number_custom() -> u32 {
            9
        }
    }

    graph! {
        #[tags("io")]
        async CustomNameGraph<graphium::Context> -> (out: u32) {
            getNumber() -> (out)
        }
    }

    let value = block_on(CustomNameGraph::__graphium_run_async(&mut ctx));
    assert_eq!(value, 9);

    let node_dto = getNumber::__graphium_dto();
    assert_eq!(node_dto.label, "getNumber");
    assert_eq!(node_dto.tags, vec!["io".to_string()]);
}

#[test]
fn e2e_graph_macro_borrows_artifacts() {
    #[derive(Default)]
    pub struct Context {
        pub number: u32,
    }

    let mut ctx = Context::default();

    node! {
        pub fn store_number(_a: u32) {
        }
    }

    node! {
        pub fn take_ownership(_ctx: &Context, a: &u32) -> u32 {
            *a
        }
    }

    graph! {
        BorrowedGraph<Context> -> (number: u32) {
            GetNumber() -> (number) >>
            StoreNumber(number) -> (&number) >>
            TakeOwnership(&number) -> (number) >>
            PipeNumber(number) -> (number)
        }
    }
    let num = BorrowedGraph::__graphium_run(&mut ctx);
    assert_eq!(num, 42);
}

#[test]
fn e2e_graph_macro_borrowed_ctx_values_persist() {
    #[derive(Default)]
    pub struct Context {
        pub number: u32,
    }

    let mut ctx = Context::default();

    node! {
        fn check_number(ctx: &Context, number: &u32) {
            assert_eq!(ctx.number, *number);
        }
    }

    node! {
        fn check_reference_expiration(ctx: &Context) {
            assert_eq!(ctx.number, 42);
        }
    }

    graph! {
        ReferenceGraph<Context> {
            GetNumber() -> (&number) >>
            CheckNumber(&number) >>
            CheckReferenceExpiration()
        }
    }

    ReferenceGraph::__graphium_run(&mut ctx);
}

#[test]
fn e2e_graph_macro_reference_can_be_forwarded() {
    #[derive(Default)]
    pub struct Context {
        pub number: u32,
    }

    let mut ctx = Context::default();

    node! {
        fn check_number(ctx: &Context, number: &u32) {
            assert_eq!(ctx.number, *number);
        }
    }

    node! {
        fn check_reference_expiration(ctx: &Context) {
            assert_eq!(ctx.number, 42);
        }
    }

    graph! {
        ReferenceGraphForwarded<Context> {
            GetNumber() -> (&number) >>
            CheckNumber(&number) -> &number >>
            CheckReferenceExpiration()
        }
    }

    ReferenceGraphForwarded::__graphium_run(&mut ctx);
}

#[test]
fn e2e_graph_macro_can_take_ownership_from_ctx() {
    #[derive(Default)]
    pub struct Context {
        pub number: u32,
    }

    let mut ctx = Context::default();

    node! {
        fn take_number(number: u32) {
            assert_eq!(number, 42);
        }
    }

    node! {
        fn assert_taken_clears_ctx(ctx: &Context) {
            assert_eq!(ctx.number, 0);
        }
    }

    graph! {
        TakeGraph<Context> {
            GetNumber() -> (&number) >>
            TakeNumber(*number) >>
            AssertTakenClearsCtx()
        }
    }

    TakeGraph::__graphium_run(&mut ctx);
}
