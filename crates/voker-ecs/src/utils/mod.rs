//! Shared utility types used by `voker-ecs`.
//!
//! This module contains two categories of helpers:
//! - Public debugging/runtime helpers re-exported for crate users.
//! - Internal building blocks used by storage, archetype, and world internals.
//!
//! # Public Exports
//! - [`DebugLocation`]: lightweight call-site metadata.
//! - [`DebugName`]: debug-oriented type/name formatter.
//! - [`Dropper`]: type-erased drop callback wrapper.
//!
//! # Internal Exports
//! Internal items are crate-private and focus on performance and safety-sensitive
//! internals (slice pools, panic guards, and SIMD-friendly helpers).

// -----------------------------------------------------------------------------
// Modules

mod debug_location;
mod debug_name;
mod debug_unwrap;
mod dropper;
mod helper;
mod ident_pool;
mod on_panic;

// -----------------------------------------------------------------------------
// Exports

pub use debug_location::DebugLocation;
pub use debug_name::DebugName;
pub use dropper::Dropper;

pub(crate) use debug_unwrap::DebugCheckedUnwrap;
pub(crate) use helper::*;
pub(crate) use ident_pool::SlicePool;
pub(crate) use on_panic::ForgetEntityOnPanic;
