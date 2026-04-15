use std::any::Any;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestKind {
    Node,
    Graph,
}

pub struct RegisteredTest {
    pub name: &'static str,
    pub target: &'static str,
    pub kind: TestKind,
    pub run: fn() -> Result<(), String>,
}

inventory::collect!(RegisteredTest);

pub fn registered_tests() -> Vec<&'static RegisteredTest> {
    inventory::iter::<RegisteredTest>.into_iter().collect()
}

pub fn panic_payload_to_string(payload: Box<dyn Any + Send>) -> String {
    if let Some(msg) = payload.downcast_ref::<&'static str>() {
        return (*msg).to_string();
    }
    if let Some(msg) = payload.downcast_ref::<String>() {
        return msg.clone();
    }
    "panic while running test".to_string()
}
