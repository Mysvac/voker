// -----------------------------------------------------------------------------
// Schedule

use crate::prelude::IntoSystemConfig;
use crate::schedule::{Schedule, ScheduleLabel};
use crate::system::IntoSystem;
use crate::world::World;

impl World {
    /// Insert a schedule to the world, return the old one if exists.
    ///
    /// If a schedule with the same label already exists, it will be replaced.
    pub fn insert_schedule(&mut self, schedule: Schedule) -> Option<Schedule> {
        self.schedules.insert(schedule)
    }

    /// Remove a schedule from the world if exists.
    ///
    /// If a schedule with the same label already exists, it will be replaced.
    pub fn remove_schedule(&mut self, label: impl ScheduleLabel) -> Option<Schedule> {
        self.schedules.remove(label.intern())
    }

    /// Returns a mutable reference to the schedule with the given label.
    ///
    /// Initializes a new empty schedule if it doesn't exist.
    pub fn schedule_entry(&mut self, label: impl ScheduleLabel) -> &mut Schedule {
        self.schedules.entry(label.intern())
    }

    /// Executes a closure with exclusive access to a schedule and the world.
    ///
    /// Initializes a new empty schedule if it doesn't exist.
    ///
    /// This method temporarily removes the schedule from the world to satisfy
    /// Rust's borrowing rules, allowing the closure to mutably borrow both the
    /// schedule and the world simultaneously.
    pub fn schedule_scope<R>(
        &mut self,
        label: impl ScheduleLabel,
        func: impl FnOnce(&mut World, &mut Schedule) -> R,
    ) -> R {
        let label = label.intern();
        let mut schedule = self.schedules.remove(label).unwrap_or_else(|| Schedule::new(label));

        let value = func(self, &mut schedule);

        let old = self.schedules.insert(schedule);

        if old.is_some() {
            log::warn!(
                "Schedule `{label:?}` was inserted during a call to \
                `World::schedule_scope`: its value has been overwritten"
            );
        }

        value
    }

    /// Executes a closure with exclusive access to a schedule and the world.
    ///
    /// If Schedule does not exist, return None directly.
    ///
    /// This method temporarily removes the schedule from the world to satisfy
    /// Rust's borrowing rules, allowing the closure to mutably borrow both the
    /// schedule and the world simultaneously.
    pub fn try_schedule_scope<R>(
        &mut self,
        label: impl ScheduleLabel,
        func: impl FnOnce(&mut World, &mut Schedule) -> R,
    ) -> Option<R> {
        let label = label.intern();
        let Some(mut schedule) = self.schedules.remove(label) else {
            return None;
        };

        let value = func(self, &mut schedule);

        let old = self.schedules.insert(schedule);

        if old.is_some() {
            log::warn!(
                "Schedule `{label:?}` was inserted during a call to \
                `World::schedule_scope`: its value has been overwritten"
            );
        }

        Some(value)
    }

    /// Runs the schedule with the given label.
    ///
    /// Initializes a new empty schedule if it doesn't exist.
    ///
    /// This is a convenience method that combines `schedule_scope`
    /// with running the schedule.
    pub fn run_schedule(&mut self, label: impl ScheduleLabel) {
        self.schedule_scope(label.intern(), |world, sched| sched.run(world));
    }

    /// Runs the schedule with the given label if exist.
    ///
    /// If Schedule does not exist, return directly.
    ///
    /// This is a convenience method that combines `try_schedule_scope`
    /// with running the schedule.
    pub fn try_run_schedule(&mut self, label: impl ScheduleLabel) {
        self.try_schedule_scope(label.intern(), |world, sched| sched.run(world));
    }

    /// Adds one system to the schedule identified by label.
    ///
    /// This function is faster then `add_systems`.
    pub fn add_system<M>(&mut self, label: impl ScheduleLabel, system: impl IntoSystem<(), (), M>) {
        self.schedules.add_system(label, system);
    }

    /// Adds one or many systems/configurations to the schedule identified by label.
    pub fn add_systems<M>(&mut self, label: impl ScheduleLabel, systems: impl IntoSystemConfig<M>) {
        self.schedules.add_systems(label, systems);
    }
}
