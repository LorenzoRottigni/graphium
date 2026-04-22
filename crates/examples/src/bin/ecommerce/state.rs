use std::sync::Arc;

use tokio::sync::Mutex;

use crate::context::Context;

#[derive(Clone)]
pub struct AppState {
    pub graphium_ctx: Arc<Mutex<Context>>,
}
