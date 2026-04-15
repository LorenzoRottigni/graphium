#[derive(Default, PartialEq, Eq, Debug, Copy, Clone)]
pub enum Status {
    #[default]
    Success,
    Fail,
    Retry,
}

#[derive(Default)]
pub struct Context {
    pub a_number: u32,
    pub status: Status,
}
