//! World-level schedule registry.
//!
//! [`Schedules`] is a [`Resource`] that maps [`ScheduleLabel`]s to their
//! [`Schedule`] instances. It provides insertion and lookup helpers and is
//! the authoritative store consulted when a world runs a schedule by label.
//!
//! [`Resource`]: crate::resource::Resource

use core::fmt::Debug;

use voker_utils::hash::HashMap;

use super::{InternedScheduleLabel, Schedule, ScheduleLabel};
use crate::schedule::IntoSystemConfig;
use crate::system::{IntoSystem, SystemSet};

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

    /// Returns the number of elements in the schedules.
    pub fn len(&self) -> usize {
        self.mapper.len()
    }

    /// Returns true if the schedules contains no elements.
    pub fn is_empty(&self) -> bool {
        self.mapper.is_empty()
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
        self.mapper.iter().map(|(label, schedule)| (&**label, schedule))
    }

    /// Returns an iterator over mutable references to all schedules. Iteration order is undefined.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&dyn ScheduleLabel, &mut Schedule)> {
        self.mapper.iter_mut().map(|(label, schedule)| (&**label, schedule))
    }

    /// Adds one or many systems into `set` on the schedule identified by `label`.
    ///
    /// All systems in `config` have their `SystemId` updated to include `set`
    /// membership. Equivalent to calling [`Schedule::add_systems`].
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn add_systems<M>(
        &mut self,
        label: impl ScheduleLabel,
        set: impl SystemSet,
        systems: impl IntoSystemConfig<M>,
    ) -> &mut Self {
        self.entry(label).add_systems(set, systems);
        self
    }

    /// Add one system.
    pub fn add_system<M>(
        &mut self,
        label: impl ScheduleLabel,
        set: impl SystemSet,
        system: impl IntoSystem<(), (), M>,
    ) -> &mut Self {
        self.entry(label).add_system(set, system);
        self
    }
}
