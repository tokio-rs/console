#![doc = include_str!("../README.md")]

/// Represents the operations performed by an async runtime.
pub mod async_ops;
/// Represents unique id's and Rust source locations.
mod common;
/// Represents interactions between the console-subscriber and a console client observing it.
pub mod instrument;
/// Represents updates to the resources in an async runtime.
pub mod resources;
/// Represents updates to the tasks in an async runtime.
pub mod tasks;
/// Represents events on the tracing subsystem: thread registration and span activities.
pub mod trace;
pub use common::*;
