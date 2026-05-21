//! Graphium documentation pages.
//!
//! This module is only compiled for rustdoc builds. It `include_str!`s the
//! Markdown files under `crates/core/docs/` so:
//! - the docs show up in generated rustdoc
//! - code blocks can be compiled as doctests

#![allow(dead_code)]

#[doc = include_str!("../docs/index.md")]
mod index {}

#[doc = include_str!("../docs/getting_started.md")]
mod getting_started {}

#[doc = include_str!("../docs/dsl.md")]
mod dsl {}

#[doc = include_str!("../docs/artifacts.md")]
mod artifacts {}

#[doc = include_str!("../docs/nodes.md")]
mod nodes {}

#[doc = include_str!("../docs/graphs.md")]
mod graphs {}

#[doc = include_str!("../docs/control_flow.md")]
mod control_flow {}

#[doc = include_str!("../docs/async.md")]
mod async_graphs {}

#[doc = include_str!("../docs/features.md")]
mod features {}

#[doc = include_str!("../docs/telemetry.md")]
mod telemetry {}

#[doc = include_str!("../docs/testing.md")]
mod testing {}

#[doc = include_str!("../docs/dashboard.md")]
mod dashboard {}

