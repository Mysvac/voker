use core::fmt::Debug;

use super::{Direction, GraphNode};
use crate::system::{AccessTable, SystemFlags, SystemId};
use crate::world::{DeferredWorld, World};

// -----------------------------------------------------------------------------
// SystemKey

slotmap::new_key_type! {
    /// Stable key used to identify a system node in the schedule graph.
    pub struct SystemKey;
}

impl GraphNode for SystemKey {
    type Link = (SystemKey, Direction);
    type Edge = (SystemKey, SystemKey);
}

// -----------------------------------------------------------------------------
// SystemObject

use super::{ActionSystem, ConditionSystem};

/// Runtime bundle of an erased system and its access metadata.
///
/// `access` is filled during initialization and later used by
/// the scheduler to validate conflicts and build execution order.
pub enum SystemObject {
    Action {
        system: ActionSystem,
        access: AccessTable,
    },
    Condition {
        system: ConditionSystem,
        access: AccessTable,
    },
}

impl SystemObject {
    /// Creates a new `SystemObject` from an `ActionSystem`.
    #[inline]
    pub fn new_action(system: ActionSystem) -> Self {
        Self::Action {
            system,
            access: AccessTable::new(),
        }
    }

    /// Creates a new `SystemObject` from a `ConditionSystem`.
    #[inline]
    pub fn new_condition(system: ConditionSystem) -> Self {
        Self::Condition {
            system,
            access: AccessTable::new(),
        }
    }

    /// Returns the unique identifier of the underlying system.
    #[inline]
    pub fn id(&self) -> SystemId {
        match self {
            SystemObject::Action { system, .. } => system.id(),
            SystemObject::Condition { system, .. } => system.id(),
        }
    }

    /// Returns the flags associated with the underlying system.
    #[inline]
    pub fn flags(&self) -> SystemFlags {
        match self {
            SystemObject::Action { system, .. } => system.flags(),
            SystemObject::Condition { system, .. } => system.flags(),
        }
    }

    /// Returns `true` if the underlying system has deferred operations.
    #[inline]
    pub fn is_deferred(&self) -> bool {
        match self {
            SystemObject::Action { system, .. } => system.is_deferred(),
            SystemObject::Condition { system, .. } => system.is_deferred(),
        }
    }

    /// Returns `true` if the underlying system requires exclusive world access.
    #[inline]
    pub fn is_exclusive(&self) -> bool {
        match self {
            SystemObject::Action { system, .. } => system.is_exclusive(),
            SystemObject::Condition { system, .. } => system.is_exclusive(),
        }
    }

    /// Returns `true` if the underlying system is `!Send`.
    #[inline]
    pub fn is_non_send(&self) -> bool {
        match self {
            SystemObject::Action { system, .. } => system.is_non_send(),
            SystemObject::Condition { system, .. } => system.is_non_send(),
        }
    }

    /// Defers the execution of the underlying system using the given `DeferredWorld`.
    #[inline]
    pub fn defer(&mut self, world: DeferredWorld) {
        match self {
            SystemObject::Action { system, .. } => system.defer(world),
            SystemObject::Condition { system, .. } => system.defer(world),
        }
    }

    /// Applies any deferred operations from the underlying system to the `World`.
    #[inline]
    pub fn apply_deferred(&mut self, world: &mut World) {
        match self {
            SystemObject::Action { system, .. } => system.apply_deferred(world),
            SystemObject::Condition { system, .. } => system.apply_deferred(world),
        }
    }
}
