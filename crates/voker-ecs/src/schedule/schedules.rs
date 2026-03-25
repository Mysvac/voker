use core::fmt::Debug;

use voker_utils::hash::HashMap;

use super::{InternedScheduleLabel, Schedule, ScheduleLabel, UnitSystem};
use crate::resource::Resource;
use crate::system::{IntoSystem, SystemName};

// -----------------------------------------------------------------------------
// Schedules

/// A registry of schedules indexed by schedule label.
///
/// This resource provides management APIs for creating, retrieving, and
/// mutating multiple schedules, and for inserting/removing systems in a
/// label-scoped way.
pub struct Schedules {
    mapper: HashMap<InternedScheduleLabel, Schedule>,
}

impl Resource for Schedules {}

impl Debug for Schedules {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.mapper.values()).finish()
    }
}

impl Default for Schedules {
    fn default() -> Self {
        Self::new()
    }
}

impl Schedules {
    /// Creates an empty schedule registry.
    pub const fn new() -> Self {
        Self {
            mapper: HashMap::new(),
        }
    }

    /// Inserts a schedule by its label.
    ///
    /// Returns the previous schedule with the same label, if any.
    pub fn insert(&mut self, schedule: Schedule) -> Option<Schedule> {
        self.mapper.insert(schedule.label(), schedule)
    }

    /// Removes and returns the schedule for `label`, if it exists.
    pub fn remove(&mut self, label: impl ScheduleLabel) -> Option<Schedule> {
        self.mapper.remove(&label.intern())
    }

    /// Returns `true` if a schedule with `label` already exists.
    pub fn contains(&self, label: impl ScheduleLabel) -> bool {
        self.mapper.contains_key(&label.intern())
    }

    /// Returns a reference to the schedule associated with `label`, if it exists.
    pub fn get(&self, label: impl ScheduleLabel) -> Option<&Schedule> {
        self.mapper.get(&label.intern())
    }

    /// Returns a mutable reference to the schedule associated with `label`, if it exists.
    pub fn get_mut(&mut self, label: impl ScheduleLabel) -> Option<&mut Schedule> {
        self.mapper.get_mut(&label.intern())
    }

    /// Returns a mutable reference to the schedule associated with `label`,
    /// creating one if it doesn't already exist.
    pub fn entry(&mut self, label: impl ScheduleLabel) -> &mut Schedule {
        self.mapper
            .entry(label.intern())
            .or_insert_with(|| Schedule::new(label))
    }

    /// Returns an iterator over all schedules. Iteration order is undefined.
    pub fn iter(&self) -> impl Iterator<Item = (&dyn ScheduleLabel, &Schedule)> {
        self.mapper
            .iter()
            .map(|(label, schedule)| (&**label, schedule))
    }

    /// Returns an iterator over mutable references to all schedules. Iteration order is undefined.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&dyn ScheduleLabel, &mut Schedule)> {
        self.mapper
            .iter_mut()
            .map(|(label, schedule)| (&**label, schedule))
    }

    /// Inserts a system into the schedule identified by `label`.
    ///
    /// - Returns `true` if this inserted a new system name.
    /// - Returns `false` if an existing system with the same name was replaced.
    ///
    /// # Panics
    /// Panics if the number of systems in the target schedule exceeds `u16::MAX`.
    pub fn insert_system(&mut self, label: impl ScheduleLabel, system: UnitSystem) -> bool {
        self.entry(label).insert(system)
    }

    /// Removes a system from the schedule identified by `label`.
    ///
    /// - Returns `false` if the system does not exist.
    /// - Returns `true` if the system existed and was removed.
    pub fn remove_system(&mut self, label: impl ScheduleLabel, name: SystemName) -> bool {
        self.entry(label).remove(name)
    }

    /// Adds an explicit ordering edge: `before -> after`.
    ///
    /// Returns `false` if either system name is not present.
    ///
    /// If the edge already exists, this is idempotent.
    pub fn insert_order(
        &mut self,
        label: impl ScheduleLabel,
        before: SystemName,
        after: SystemName,
    ) -> bool {
        self.entry(label).insert_order(before, after)
    }

    /// Removes an explicit ordering edge: `before -> after`.
    ///
    /// Returns `false` if either system name is not present or the order is not present.
    pub fn remove_order(
        &mut self,
        label: impl ScheduleLabel,
        before: SystemName,
        after: SystemName,
    ) -> bool {
        self.entry(label).remove_order(before, after)
    }

    pub fn add_system<S, M>(&mut self, label: impl ScheduleLabel, system: S) -> &mut Self
    where
        S: IntoSystem<(), (), M>,
    {
        self.entry(label).add_system(system);
        self
    }

    pub fn del_system<S, M>(&mut self, label: impl ScheduleLabel, system: S) -> &mut Self
    where
        S: IntoSystem<(), (), M>,
    {
        self.entry(label).del_system(system);
        self
    }

    pub fn add_order<X, Y, M1, M2>(
        &mut self,
        label: impl ScheduleLabel,
        before: X,
        after: Y,
    ) -> &mut Self
    where
        X: IntoSystem<(), (), M1>,
        Y: IntoSystem<(), (), M2>,
    {
        self.entry(label).add_order(before, after);
        self
    }

    pub fn del_order<X, Y, M1, M2>(
        &mut self,
        label: impl ScheduleLabel,
        before: X,
        after: Y,
    ) -> &mut Self
    where
        X: IntoSystem<(), (), M1>,
        Y: IntoSystem<(), (), M2>,
    {
        self.entry(label).del_order(before, after);
        self
    }
}
