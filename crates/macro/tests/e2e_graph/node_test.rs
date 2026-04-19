use graphium;
use graphium_macro::{graph, graph_test, node};

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
    #[metadata(context = graphium::Context, outputs = (result: u32))]
    TestableGraph {
        GetNumber() -> (number) >>
        PipeNumber(number) -> (result)
    }
}

graph_test! {
    #[test]
    fn e2e_graph_test_supports_standard_test_items() {
        let mut ctx = graphium::Context::default();
        let out = TestableGraph::__graphium_run(&mut ctx);
        assert_eq!(out, 42);
    }
}

graph_test! {
    #[test]
    fn e2e_graph_test_supports_standard_test_items_second() {
        let mut ctx = graphium::Context::default();
        let out = TestableGraph::__graphium_run(&mut ctx);
        assert!(out > 0);
    }
}
