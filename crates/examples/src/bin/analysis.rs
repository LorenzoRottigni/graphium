use graphium::{graph, node};

pub fn main() {
    let mut ctx = graphium::Context::default();

    node! {
        fn duplicate(a: u32) -> (u32, u32) {
            (a, a)
        }
    }

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
        #[metrics("performance")]
        OwnedGraph -> (a_split: u32) {
            GetNumber() -> (number) >>
            Duplicate(number) -> (a_split, b_split) >>
            PipeNumber(a_split) -> (a_split)
        }
    }

    let duplicated = OwnedGraph::run(&mut ctx);
    assert_eq!(duplicated, 42);
}
