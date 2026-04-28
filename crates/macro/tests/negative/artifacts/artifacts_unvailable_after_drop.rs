use graphium_macro::{graph, node};

/// This test ensures that artifacts cant no longer be taken by reference after being dropped:
/// - `GetNumber` produces a `number` artifact that is persisted in the graph lifetime.
/// - `PipeNumber` takes ownership of `number` moving it out of the graph lifetime.
/// - `PipeNumber` expects to take `number` by reference from the graph lifetime but it isn't available anymore
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
        InvalidGraph {
            GetNumber() -> (&number) >>
            PipeNumber(*number) >>
            PipeNumber(&number) -> (number)
        }
    }
}
