//! Provides underlying IO interfaces.

pub mod embedded;
pub mod future;
mod gated;
pub mod memory;
mod reader;
mod source;
pub mod watcher;
mod writer;

pub use embedded::EMBEDDED;
pub use reader::*;
pub use source::*;
pub use writer::*;

#[cfg(target_os = "android")]
pub mod android;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub mod file;

#[cfg(any(feature = "http", feature = "https"))]
pub mod web;
