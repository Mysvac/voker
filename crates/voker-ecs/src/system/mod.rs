//! System abstraction, metadata, and parameter extraction pipeline.
//!
//! This module includes:
//! - function-to-system conversion traits,
//! - runtime system metadata and flags,
//! - access tracking structures used by schedule conflict checks,
//! - input/parameter extraction (`SystemInput`, `SystemParam`).
//!
//! # Execution model at a glance
//!
//! During system initialization:
//! 1. each [`SystemParam`] registers access into an [`AccessTable`],
//! 2. the schedule consumes those tables to build conflict constraints.
//!
//! During system execution:
//! 1. scheduler chooses systems whose dependencies and conflicts allow running,
//! 2. system parameters are fetched with the world/state contract,
//! 3. deferred effects are flushed when required.
//!
//! This split keeps conflict checking deterministic and cheap at runtime:
//! expensive structural reasoning happens once during (re)initialization.

// -----------------------------------------------------------------------------
// Modules

mod access;
mod error;
mod function;
mod ident;
mod input;
mod meta;
mod param;
mod set;
mod system;

// -----------------------------------------------------------------------------
// Exports

pub use error::*;

pub use access::{AccessParam, AccessTable, FilterParam, FilterParamBuilder};
pub use function::{FunctionSystem, SystemFunction};
pub use ident::SystemId;
pub use input::{In, InMut, InRef, SystemInput};
pub use meta::{SystemFlags, SystemMeta};
pub use param::{Local, NonSendMarker, SystemParam, SystemTick};
pub use set::{InternedSystemSet, SystemSet, SystemSetBegin, SystemSetEnd};
pub use system::{IntoMapSystem, IntoPipeSystem};
pub use system::{IntoSystem, MapSystem, PipeSystem, System};
pub use voker_ecs_derive::SystemParam;
