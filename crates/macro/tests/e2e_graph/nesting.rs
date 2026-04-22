use graphium_macro::{graph, node};

#[derive(Default)]
struct TestContext;

node! {
    fn identity_pass(a: u32, b: u32) -> (u32, u32) {
        (a, b)
    }
}

node! {
    fn add_numbers(a: u32, b: u32) -> u32 {
        a + b
    }
}

node! {
    fn multiply_numbers(a: u32, b: u32) -> u32 {
        a * b
    }
}

node! {
    fn duplicate_value(value: u32) -> (u32, u32) {
        (value, value)
    }
}

node! {
    fn add_ten(value: u32) -> u32 {
        value + 10
    }
}

node! {
    fn split_sum(left: u32, right: u32) -> (u32, u32, u32) {
        (left, right, left + right)
    }
}

node! {
    fn quad_split(value: u32) -> (u32, u32, u32, u32) {
        (value, value, value, value)
    }
}

node! {
    fn constant_value() -> u32 {
        42
    }
}

graph! {
    #[metadata(context = TestContext)]
    InnerPassthrough(a: u32, b: u32) -> (a: u32, b: u32) {
        IdentityPass(a, b) -> (a, b)
    }
}

graph! {
    #[metadata(context = TestContext)]
    InnerSum(a: u32, b: u32) -> (sum: u32) {
        AddNumbers(a, b) -> (sum)
    }
}

graph! {
    #[metadata(context = TestContext)]
    InnerProduct(a: u32, b: u32) -> (product: u32) {
        MultiplyNumbers(a, b) -> (product)
    }
}

graph! {
    #[metadata(context = TestContext)]
    InnerAddBoth(a: u32, b: u32) -> (a: u32, b: u32) {
        AddTen(a) -> (a) & AddTen(b) -> (b)
    }
}

graph! {
    #[metadata(context = TestContext)]
    InnerDuplicate(value: u32) -> (a: u32, b: u32) {
        DuplicateValue(value) -> (a, b)
    }
}

graph! {
    #[metadata(context = TestContext)]
    InnerSplitSum(a: u32, b: u32) -> (a: u32, b: u32, sum: u32) {
        SplitSum(a, b) -> (a, b, sum)
    }
}

graph! {
    #[metadata(context = TestContext)]
    OuterGraphWithPassthrough(value: u32) -> (a: u32, b: u32) {
        DuplicateValue(value) -> (a, b) >>
        InnerPassthrough::run(a, b) -> (a, b)
    }
}

graph! {
    #[metadata(context = TestContext)]
    OuterGraphWithNestedSum(value: u32) -> (sum: u32) {
        DuplicateValue(value) -> (a, b) >>
        InnerSum::run(a, b) -> (sum)
    }
}

graph! {
    #[metadata(context = TestContext)]
    OuterGraphWithNestedProduct(a: u32, b: u32) -> (result: u32) {
        InnerProduct::run(a, b) -> (product) >>
        AddNumbers(product, product) -> (result)
    }
}

graph! {
    #[metadata(context = TestContext)]
    OuterGraphWithAddBoth(a: u32, b: u32) -> (a: u32, b: u32) {
        InnerAddBoth::run(a, b) -> (a, b)
    }
}

graph! {
    #[metadata(context = TestContext)]
    OuterGraphWithSplitSum(a: u32, b: u32) -> (a: u32, b: u32, sum: u32) {
        InnerSplitSum::run(a, b) -> (a, b, sum)
    }
}

graph! {
    #[metadata(context = TestContext)]
    DeepNestingLevel1(value: u32) -> (sum: u32) {
        DuplicateValue(value) -> (a, b) >>
        InnerSum::run(a, b) -> (sum)
    }
}

graph! {
    #[metadata(context = TestContext)]
    DeepNestingLevel2(value: u32) -> (sum: u32) {
        DeepNestingLevel1::run(value) -> (inner_sum) >>
        DuplicateValue(inner_sum) -> (a, b) >>
        InnerSum::run(a, b) -> (sum)
    }
}

graph! {
    #[metadata(context = TestContext)]
    TripleNesting(value: u32) -> (sum: u32) {
        DuplicateValue(value) -> (a, b) >>
        InnerSum::run(a, b) -> (sum1) >>
        MultiplyNumbers(sum1, sum1) -> (sum)
    }
}

graph! {
    #[metadata(context = TestContext)]
    ChainedNestedGraphs(a: u32, b: u32) -> (a: u32, b: u32) {
        InnerPassthrough::run(a, b) -> (a1, b1) >>
        InnerPassthrough::run(a1, b1) -> (a, b)
    }
}

graph! {
    #[metadata(context = TestContext)]
    InnerQuad(v: u32) -> (a: u32, b: u32, c: u32, d: u32) {
        QuadSplit(v) -> (a, b, c, d)
    }
}

graph! {
    #[metadata(context = TestContext)]
    InnerConstant -> (value: u32) {
        ConstantValue() -> (value)
    }
}

#[test]
fn e2e_nested_graph_passthrough_preserves_values() {
    let mut ctx = TestContext::default();
    let (a, b) = InnerPassthrough::__graphium_run(&mut ctx, 5, 10);
    assert_eq!(a, 5);
    assert_eq!(b, 10);
}

#[test]
fn e2e_nested_graph_with_single_output() {
    let mut ctx = TestContext::default();
    let sum = InnerSum::__graphium_run(&mut ctx, 3, 7);
    assert_eq!(sum, 10);
}

#[test]
fn e2e_outer_graph_calls_inner_graph() {
    let mut ctx = TestContext::default();
    let (a, b) = OuterGraphWithPassthrough::__graphium_run(&mut ctx, 42);
    assert_eq!(a, 42);
    assert_eq!(b, 42);
}

#[test]
fn e2e_outer_graph_transforms_via_inner() {
    let mut ctx = TestContext::default();
    let sum = OuterGraphWithNestedSum::__graphium_run(&mut ctx, 15);
    assert_eq!(sum, 30);
}

#[test]
fn e2e_outer_graph_uses_inner_product() {
    let mut ctx = TestContext::default();
    let result = OuterGraphWithNestedProduct::__graphium_run(&mut ctx, 4, 5);
    assert_eq!(result, 40);
}

#[test]
fn e2e_inner_add_both_transforms_both() {
    let mut ctx = TestContext::default();
    let (a, b) = OuterGraphWithAddBoth::__graphium_run(&mut ctx, 3, 7);
    assert_eq!(a, 13);
    assert_eq!(b, 17);
}

#[test]
fn e2e_inner_add_both_direct_call() {
    let mut ctx = TestContext::default();
    let (a, b) = InnerAddBoth::__graphium_run(&mut ctx, 3, 7);
    assert_eq!(a, 13);
    assert_eq!(b, 17);
}

#[test]
fn e2e_inner_split_sum_returns_all_values() {
    let mut ctx = TestContext::default();
    let (a, b, sum) = OuterGraphWithSplitSum::__graphium_run(&mut ctx, 6, 9);
    assert_eq!(a, 6);
    assert_eq!(b, 9);
    assert_eq!(sum, 15);
}

#[test]
fn e2e_deep_nesting_level_1() {
    let mut ctx = TestContext::default();
    let sum = DeepNestingLevel1::__graphium_run(&mut ctx, 20);
    assert_eq!(sum, 40);
}

#[test]
fn e2e_deep_nesting_level_2() {
    let mut ctx = TestContext::default();
    let sum = DeepNestingLevel2::__graphium_run(&mut ctx, 20);
    assert_eq!(sum, 80);
}

#[test]
fn e2e_triple_nesting() {
    let mut ctx = TestContext::default();
    let sum = TripleNesting::__graphium_run(&mut ctx, 10);
    assert_eq!(sum, 400);
}

#[test]
fn e2e_chained_nested_graphs() {
    let mut ctx = TestContext::default();
    let (a, b) = ChainedNestedGraphs::__graphium_run(&mut ctx, 7, 14);
    assert_eq!(a, 7);
    assert_eq!(b, 14);
}

#[test]
fn e2e_nested_graph_multiple_calls_in_sequence() {
    let mut ctx = TestContext::default();
    let (a1, b1) = InnerPassthrough::__graphium_run(&mut ctx, 1, 2);
    let (a2, b2) = InnerPassthrough::__graphium_run(&mut ctx, a1, b1);
    let (a3, b3) = InnerPassthrough::__graphium_run(&mut ctx, a2, b2);
    assert_eq!(a3, 1);
    assert_eq!(b3, 2);
}

#[test]
fn e2e_nested_graph_with_zero_inputs() {
    let mut ctx = TestContext::default();
    let value = InnerConstant::__graphium_run(&mut ctx);
    assert_eq!(value, 42);
}

#[test]
fn e2e_nested_graph_with_four_outputs() {
    let mut ctx = TestContext::default();
    let (a, b, c, d) = InnerQuad::__graphium_run(&mut ctx, 7);
    assert_eq!(a, 7);
    assert_eq!(b, 7);
    assert_eq!(c, 7);
    assert_eq!(d, 7);
}

#[test]
fn e2e_nested_graph_with_partial_output_usage() {
    let mut ctx = TestContext::default();
    let (a, _b, sum) = OuterGraphWithSplitSum::__graphium_run(&mut ctx, 8, 12);
    assert_eq!(a, 8);
    assert_eq!(sum, 20);
}
