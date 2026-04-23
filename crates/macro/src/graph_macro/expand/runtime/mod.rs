//! Runtime code generation helpers for graph execution.
//!
//! This module groups synchronous and asynchronous runtime implementation
//! generators behind a single public interface.

mod r#async;
mod sync;

pub(super) use r#async::build_async_impl;
pub(super) use sync::build_sync_impl;
