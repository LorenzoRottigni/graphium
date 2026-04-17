use graphium;
use graphium_macro::{graph, node};

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
        fn duplicate(a: u32) -> (u32, u32) {
            (a, a)
        }
    }

    graph! {
        #[metadata(outputs = (a_split: u32))]
        OwnedGraph {
            GetNumber() -> (number) >>
            Duplicate(number) -> (a_split, b_split) >>
            PipeNumber(a_split) -> (a_split)
        }
    }
    let duplicated = OwnedGraph::__graphium_run(&mut ctx);

    assert_eq!(duplicated, 42);
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
        #[metadata(context = Context, outputs = (number: u32))]
        BorrowedGraph {
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
#[should_panic]
fn e2e_graph_macro_reference_last_1_hop() {
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
        #[metadata(context = Context)]
        ReferenceGraph {
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
        #[metadata(context = Context)]
        ReferenceGraphForwarded {
            GetNumber() -> (&number) >>
            CheckNumber(&number) -> &number >>
            CheckReferenceExpiration()
        }
    }

    ReferenceGraphForwarded::__graphium_run(&mut ctx);
}
