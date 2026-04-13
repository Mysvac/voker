//! Scheduling and system execution pipeline.
//!
//! This module contains:
//! - schedule labels and schedule collections,
//! - dependency graph utilities,
//! - system ordering/concurrency planning,
//! - executor backends (single-threaded and multi-threaded).
//!
//! Deferred world mutations can be synchronized with [`ApplyDeferred`] (or the
//! [`apply_deferred`] helper), which introduces an explicit point where pending
//! deferred buffers may be applied before subsequent systems continue.

// -----------------------------------------------------------------------------
// Modules

mod apply;
mod config;
mod executor;
mod graph;
mod label;
mod schedule;
mod schedules;
mod system;

// -----------------------------------------------------------------------------
// Alias

use crate::system::System;
use alloc::boxed::Box;

// -----------------------------------------------------------------------------
// Exports

pub use voker_ecs_derive::ScheduleLabel;

pub use executor::{ExecutorKind, MainThreadExecutor, SystemExecutor};
pub use executor::{MultiThreadedExecutor, SingleThreadedExecutor};
pub use graph::{Dag, DiGraph, ToposortError, UnGraph};
pub use graph::{Direction, Graph, GraphNode, SccIterator, SccNodes};
pub use label::{AnonymousSchedule, InternedScheduleLabel, ScheduleLabel};
pub use schedule::{Schedule, SystemSchedule};
pub use schedules::Schedules;

pub use system::{SystemKey, SystemObject};

pub use apply::{ApplyDeferred, apply_deferred};

pub type ConditionSystem = Box<dyn System<Input = (), Output = bool>>;
pub type ActionSystem = Box<dyn System<Input = (), Output = ()>>;
