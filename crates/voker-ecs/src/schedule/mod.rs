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

mod executor;
mod graph;
mod label;
mod schedule;
mod schedules;
mod system;

pub mod config;

// -----------------------------------------------------------------------------
// Alias

use crate::system::System;
use alloc::boxed::Box;
use config::SystemNode;

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

pub use apply::{ApplyDeferred, apply_deferred, apply_deferred_of_val};
pub use config::{IntoSystemConfig, SystemConfig};

pub type ConditionSystem = Box<dyn System<Input = (), Output = bool>>;
pub type ActionSystem = Box<dyn System<Input = (), Output = ()>>;
