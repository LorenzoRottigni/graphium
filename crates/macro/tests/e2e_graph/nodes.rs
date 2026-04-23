use graphium_macro::{node, node_test};

node! {
    fn testable_add(left: u32, right: u32) -> u32 {
        left + right
    }
}

node_test! {
    #[test]
    fn e2e_node_test_supports_standard_test_items() {
        let out = TestableAdd::run(&(), 20, 22);
        assert_eq!(out, 42);
    }
}

node_test! {
    #[test]
    fn e2e_node_test_supports_args(node: &TestableAdd, left: u32, right: u32) {
        let out = node::run(&(), left, right);
        assert_eq!(out, left + right);
    }
}
