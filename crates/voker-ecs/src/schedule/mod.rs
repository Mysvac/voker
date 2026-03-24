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
// Exports

pub use voker_ecs_derive::ScheduleLabel;

pub use executor::{ExecutorKind, MainThreadExecutor, SystemExecutor};
pub use executor::{MultiThreadedExecutor, SingleThreadedExecutor};
pub use graph::{Dag, DiGraph, ToposortError, UnGraph};
pub use graph::{Direction, Graph, GraphNode, SccIterator, SccNodes};
pub use label::{AnonymousSchedule, InternedScheduleLabel, ScheduleLabel};
pub use schedule::{Schedule, SystemSchedule};
pub use schedules::Schedules;
pub use system::{SystemKey, SystemObject, UnitSystem};

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::Component;
    use crate::query::With;
    use crate::world::World;
    use alloc::string::String;
    use alloc::vec::Vec;

    #[derive(Component, Debug, PartialEq, Eq)]
    struct Foo;

    #[derive(Component, Debug, PartialEq, Eq)]
    struct Bar(u64);

    #[derive(Component, Debug, PartialEq, Eq)]
    #[component(storage = "sparse")]
    struct Baz(String);

    #[derive(Component, Debug, PartialEq)]
    struct Qux(f32);

    #[derive(Component, Debug, PartialEq, Eq)]
    struct Zaz(i32);

    #[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
    pub struct Testing;

    #[test]
    fn basic() {
        let mut world = World::default();
        let mut schedules = Schedules::new();

        schedules.add_system(Testing, spawn_entities);
        schedules.entry(Testing).run(&mut world);

        let query = world.query_with::<&Foo, With<Zaz>>();
        assert_eq!(query.iter().count(), 1);

        let query = world.query::<&Qux>();
        let qux_values: Vec<f32> = query.iter().map(|q| q.0).collect();
        assert!(qux_values.contains(&3.0));
    }

    fn spawn_entities(world: &mut World) -> () {
        world.spawn((Foo, Bar(100), Baz(String::from("a")), Qux(1.0)));
        world.spawn((Foo, Bar(200), Baz(String::from("b"))));
        world.spawn((Foo, Bar(300), Qux(3.0)));
        world.spawn((Foo, Baz(String::from("c")), Qux(4.0)));
        world.spawn((Foo, Zaz(42)));
    }
}
