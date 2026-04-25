//! Provides Sync Cell
//!
//! - [`SyncCell`]: A reimplementation of unstable [`core::sync::Exclusive`]
//! - [`SyncUnsafeCell`]: A reimplementation of unstable [`core::cell::SyncUnsafeCell`]

mod sync_cell;
mod sync_unsafe_cell;

pub use sync_cell::SyncCell;
pub use sync_unsafe_cell::SyncUnsafeCell;
