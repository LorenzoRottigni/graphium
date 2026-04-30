use graphium_macro::{graph, node};

/// This test ensures artifacts must explicitly be propagated to be moved:
/// - `GetNumber` produces a `number` artifact moved to next node.
/// - `PipeNumber` moves-in `number` artifact without explicitly propagating it.
/// - `PipeNumber` expects to move-in a `number` artifact that isn't exposed by its previous node.
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
            GetNumber() -> (number) >>
            PipeNumber(number) >>
            PipeNumber(number) // ❌ number already moved
        }
    }
}
