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
// 
// #[test]
// fn e2e_node_macro_supports_explicit_name_override() {
//     let mut ctx = graphium::Context::default();
// 
//     node! {
//         #[name = getNumber]
//         #[tags("io")]
//         async fn get_number_custom() -> u32 {
//             9
//         }
//     }
// 
//     graph! {
//         #[tags("io")]
//         async CustomNameGraph<graphium::Context> -> (out: u32) {
//             getNumber() -> (out)
//         }
//     }
// 
//     let value = block_on(CustomNameGraph::run_async(&mut ctx));
//     assert_eq!(value, 9);
// 
//     #[cfg(feature = "export")]
//     {
//         let node_dto = getNumber::dto();
//         assert_eq!(node_dto.label, "getNumber");
//         assert_eq!(node_dto.tags, vec!["io".to_string()]);
//     }
// }
