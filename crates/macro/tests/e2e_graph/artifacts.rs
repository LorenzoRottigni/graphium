use graphium_macro::{graph, node};
use graphium;

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
        #[metadata(context = graphium::Context, outputs = (a_split: u32))]
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
        pub number: u32
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
