use graphium_macro::{graph, node};

fn main() {
    node! {
        fn get_number() -> u32 {
            1
        }
    }

    node! {
        fn bump(number: &mut u32) {
            *number += 1;
        }
    }

    graph! {
        InvalidGraph<'a> {
            GetNumber() -> (&'a mut number) >>
            // Missing `mut` here should fail to type-check (`&u32` passed to `&mut u32`).
            Bump(&'a number)
        }
    }
}

