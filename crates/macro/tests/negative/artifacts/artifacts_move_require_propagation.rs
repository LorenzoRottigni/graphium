use graphium_macro::{graph, node};

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
