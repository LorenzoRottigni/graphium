use graphium_macro::{graph, node};

/// This test ensures that only artifacts borrowed from graph lifetime can be references:
/// - `GetNumber` produces a `number` artifact that is moved to next node (not persisted in graph lifetime)
/// - `PipeNumber` expects to take-in a `number` artifact by reference that should be owned by its parent graph.
/// - Expect error: `number` isn't living in the graph lifetime so can't be borrowed by reference.
fn main() {
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

    graph! {
        InvalidGraph<'a> {
            GetNumber() -> (number) >>
            PipeNumber(&'a number)
        }
    }
}
