#![warn(clippy::all, rust_2018_idioms)]

mod app;
#[cfg(target_os = "macos")]
mod observer;
mod tree;

pub use app::AxTreeApp;
#[cfg(target_os = "macos")]
pub use observer::{start_observer_background, ObserverShared};
pub use tree::{recorder::AxTreeRecorder, AxNodeData, AxNodeId, AxTreeSnapshot};
