/// This test suite contains tests that are expected to fail to compile.
/// Trybuild at runtime produces a "wip" directory with the expanded code of each test,
/// which can be used to verify that the graph macro generates the expected code and to debug compilation errors.
#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/negative/**/*.rs");
}
