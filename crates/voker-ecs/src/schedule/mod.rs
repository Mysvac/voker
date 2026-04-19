//! Scheduling and system execution pipeline.
//!
//! This module contains:
//! - schedule labels and schedule collections,
//! - system-set labels and set boundary signals,
//! - dependency graph utilities,
//! - system ordering/concurrency planning,
//! - executor backends (single-threaded and multi-threaded).
//!
//! # ApplyDeferred
//!
//! Deferred world mutations can be synchronized with [`ApplyDeferred`] (or the
//! [`apply_deferred`] helper), which introduces an explicit point where pending
//! deferred buffers may be applied before subsequent systems continue.
//!
//! # SystemSet
//!
//! A [`SystemSet`] is represented by two no-op marker systems:
//! [`SystemSetBegin`] and [`SystemSetEnd`].
//! Systems configured with [`IntoSystemConfig::in_set`] are linked by
//! run-condition edges to these markers, forming a stable boundary inside the
//! schedule graph.
//!
//! This gives set-level ordering anchors without introducing a second executor
//! or separate runtime pipeline.

// -----------------------------------------------------------------------------
// Modules

mod apply;

mod executor;
mod graph;
mod label;
mod schedule;
mod schedules;
mod set;
mod system;

pub mod config;

// -----------------------------------------------------------------------------
// Alias

use crate::system::System;
use alloc::boxed::Box;
use config::SystemNode;

// -----------------------------------------------------------------------------
// Exports

pub use voker_ecs_derive::{ScheduleLabel, SystemSet};

pub use executor::{ExecutorKind, MainThreadExecutor, SystemExecutor};
pub use executor::{MultiThreadedExecutor, SingleThreadedExecutor};
pub use graph::{Dag, DiGraph, ToposortError, UnGraph};
pub use graph::{Direction, Graph, GraphNode, SccIterator, SccNodes};
pub use label::{AnonymousSchedule, InternedScheduleLabel, ScheduleLabel};
pub use schedule::{Schedule, SystemSchedule};
pub use schedules::Schedules;
pub use set::{AnonymousSystemSet, InternedSystemSet};
pub use set::{SystemSet, SystemSetBegin, SystemSetEnd};
pub use system::{SystemKey, SystemObject};

pub use apply::{ApplyDeferred, apply_deferred, apply_deferred_of_val};
pub use config::{IntoSystemConfig, SystemConfig};

/// Boxed condition system used by schedule graphs.
pub type ConditionSystem = Box<dyn System<Input = (), Output = bool>>;
/// Boxed action system used by schedule graphs.
pub type ActionSystem = Box<dyn System<Input = (), Output = ()>>;
