//! Scheduling and system execution pipeline.
//!
//! This module contains:
//! - schedule labels and schedule collections,
//! - dependency graph utilities,
//! - system ordering/concurrency planning,
//! - executor backends (single-threaded and multi-threaded).

// -----------------------------------------------------------------------------
// Modules

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

pub type ConditionSystem = Box<dyn System<Input = (), Output = bool>>;
pub type ActionSystem = Box<dyn System<Input = (), Output = ()>>;
