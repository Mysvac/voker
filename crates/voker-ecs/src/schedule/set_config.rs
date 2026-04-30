//! System-set configuration builder.
//!
//! [`SystemSetConfig`] is a temporary structure produced by
//! [`IntoSystemSetConfig`] and consumed by [`Schedule::config_set`].
//! It captures ordering constraints and run conditions for an entire set.
//!
//! [`Schedule::config_set`]: crate::schedule::Schedule::config_set

use alloc::boxed::Box;
use alloc::vec::Vec;

use super::ConditionSystem;
use crate::system::{InternedSystemSet, IntoSystem, SystemSet};

// -----------------------------------------------------------------------------
// SystemSetConfig

/// Configuration for a [`SystemSet`] applied via
/// [`Schedule::config_set`](crate::schedule::Schedule::config_set).
pub struct SystemSetConfig {
    pub(super) set: InternedSystemSet,
    pub(super) parent: Option<InternedSystemSet>,
    pub(super) run_after: Vec<InternedSystemSet>,
    pub(super) run_before: Vec<InternedSystemSet>,
    pub(super) conditions: Vec<ConditionSystem>,
}

// -----------------------------------------------------------------------------
// IntoSystemSetConfig

/// Converts a value into a [`SystemSetConfig`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not describe a valid system set configuration",
    label = "invalid system set configuration"
)]
pub trait IntoSystemSetConfig: Sized {
    fn into_set_config(self) -> SystemSetConfig;

    /// Make this set a child of `parent`.
    ///
    /// The child set's begin and end markers are inserted into `parent`, so
    /// the child set always runs within the parent's execution window.
    #[inline]
    fn child_of(self, parent: impl SystemSet) -> SystemSetConfig {
        let mut cfg = self.into_set_config();
        cfg.parent = Some(parent.intern());
        cfg
    }

    /// Run this set after `other` ends.
    #[inline]
    fn run_after(self, other: impl SystemSet) -> SystemSetConfig {
        let mut cfg = self.into_set_config();
        cfg.run_after.push(other.intern());
        cfg
    }

    /// Run this set before `other` begins.
    #[inline]
    fn run_before(self, other: impl SystemSet) -> SystemSetConfig {
        let mut cfg = self.into_set_config();
        cfg.run_before.push(other.intern());
        cfg
    }

    /// Gate the entire set on a condition.
    ///
    /// The condition is wired to the set's begin marker. If the condition
    /// evaluates to false, begin does not run, and all systems in the set
    /// are skipped.
    #[inline]
    fn run_if<M>(self, cond: impl IntoSystem<(), bool, M>) -> SystemSetConfig {
        let mut cfg = self.into_set_config();
        cfg.conditions.push(Box::new(IntoSystem::into_system(cond)));
        cfg
    }
}

impl<S: SystemSet> IntoSystemSetConfig for S {
    fn into_set_config(self) -> SystemSetConfig {
        SystemSetConfig {
            set: self.intern(),
            parent: None,
            run_after: Vec::new(),
            run_before: Vec::new(),
            conditions: Vec::new(),
        }
    }
}

impl IntoSystemSetConfig for SystemSetConfig {
    #[inline(always)]
    fn into_set_config(self) -> SystemSetConfig {
        self
    }
}
