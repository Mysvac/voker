//! Schedule label definition and interning.
//!
//! [`ScheduleLabel`] identifies a schedule within a [`Schedules`] collection.
//! Labels are interned so comparison and hashing use pointer equality on the
//! interned value rather than full structural comparison.
//!
//! [`Schedules`]: crate::schedule::Schedules

use voker_ecs_derive::ScheduleLabel;

use crate::define_label;
use crate::label::Interned;

// -----------------------------------------------------------------------------
// ScheduleLabel

define_label!(
    /// A strongly-typed class of labels used to identify a `Schedule`.
    ///
    /// Each schedule in a `World` has a unique schedule label value, and
    /// schedules can be automatically created from labels via `Schedules::add_systems()`.
    ///
    /// Prefer defining your own label enums/structs with
    /// `#[derive(ScheduleLabel)]` for stable, explicit schedule routing.
    #[diagnostic::on_unimplemented(
        note = "consider annotating `{Self}` with `#[derive(ScheduleLabel)]`"
    )]
    ScheduleLabel,
    SCHEDULE_LABEL_INTERNER
);

/// A shorthand for `Interned<dyn ScheduleLabel>`.
pub type InternedScheduleLabel = Interned<dyn ScheduleLabel>;

#[derive(ScheduleLabel, Clone, Copy, Debug, Hash, PartialEq, Eq)]
/// Built-in label used by `Schedule::default()` for anonymous schedules.
pub struct AnonymousSchedule;
