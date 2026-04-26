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
/// This test ensures that graph macro **moves** artifacts produced by nodes using smart injection:
/// - `GetNumber` produces `number` artifact that is moved into `Duplicate`
/// - `Duplicate` produces `a_split` and `b_split` artifacts, where `a_split` is moved into `PipeNumber` and `b_split` is dropped
/// - `PipeNumber` produces `a_split` artifact that is moved out of the graph as the final output
fn e2e_graph_macro_moves_artifacts() {
    node! {
        fn duplicate(a: u32) -> (u32, u32) {
            (a, a)
        }
    }

    graph! {
        OwnedGraph -> (a_split: u32) {
            GetNumber() -> (number) >>
            Duplicate(number) -> (a_split, b_split) >>
            PipeNumber(a_split) -> (a_split)
        }
    }
    let duplicated = OwnedGraph::run_default();

    assert_eq!(duplicated, 42);
}


#[test]
/// This test ensures that graph macro **borrows** artifacts when using smart injection with references:
/// - `GetNumber` produces `number` artifact that is moved into `StoreNumber`
/// - `StoreNumber` returning `&number`, gives ownership of `number` back to the graph
/// - `TakeOwnership` graph borrows `&number` to `TakeOwnership` which unpacks the reference and returns `number` as an owned value
/// - `PipeNumber` takes ownership of `number` and moves it out of the graph as the final output
fn e2e_graph_macro_borrows_artifacts() {
    node! {
        pub fn store_number(_a: u32) {
        }
    }

    node! {
        pub fn take_ownership(a: &u32) -> u32 {
            *a
        }
    }

    graph! {
        BorrowedGraph -> (number: u32) {
            GetNumber() -> (number) >>
            StoreNumber(number) -> (&number) >>
            TakeOwnership(&number) -> (number) >>
            PipeNumber(number) -> (number)
        }
    }
    let num = BorrowedGraph::run_default();
    assert_eq!(num, 42);
}

#[test]
/// This test ensure that artifacts returned only once using `&` keep living in the graph lifetime
/// and can be borrowed multiple times from next nodes.
/// - `GetNumber` produces `number` artifact and gives its ownership to the graph
/// - `CheckNumber` borrows `&number` and checks its value without explictly propagating it
/// - `CheckReferenceStillAvailable` borrows `&number`once again ensuring that reference it's still valid
fn e2e_graph_macro_borrowed_ctx_values_persist() {
    node! {
        fn check_number(number: &u32) {
            assert_eq!(*number, 42);
        }
    }

    node! {
        fn check_reference_still_available(number: &u32) {
            assert_eq!(*number, 42);
        }
    }

    graph! {
        ReferenceGraph {
            GetNumber() -> (&number) >>
            CheckNumber(&number) >>
            CheckReferenceStillAvailable(&number)
        }
    }

    ReferenceGraph::run_default();
}

#[test]
/// This test ensures that graph macro can drop artifacts produced by nodes using `*` token:
/// - `GetNumber` produces `number` artifact and gives its ownership to the graph
/// - `TakeNumber` takes ownership of `number` moving it out of the graph and making it unavailable for next nodes
fn e2e_graph_macro_can_move_artifacts_back_to_its_nodes() {
    node! {
        fn take_number(number: u32) -> u32 {
            assert_eq!(number, 42);
            number
        }
    }

    graph! {
        TakeGraph -> (out: u32) {
            GetNumber() -> (&number) >>
            TakeNumber(*number) -> (out)
        }
    }

    let out = TakeGraph::run_default();
    assert_eq!(out, 42);
}
