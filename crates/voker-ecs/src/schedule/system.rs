use alloc::boxed::Box;
use core::fmt::Debug;

use super::{Direction, GraphNode};
use crate::system::{AccessTable, System};

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

/// Erased system type used by the scheduler runtime.
///
/// Schedules execute systems as unit tasks with no explicit input/output,
/// so concrete system signatures are normalized into this boxed trait object.
pub type UnitSystem = Box<dyn System<Input = (), Output = ()>>;

/// Runtime bundle of an erased system and its access metadata.
///
/// `access` is filled during initialization and later used by
/// the scheduler to validate conflicts and build execution order.
pub struct SystemObject {
    pub system: UnitSystem,
    pub access: AccessTable,
}

impl SystemObject {
    /// Creates a system object with an empty access table.
    ///
    /// The returned value is uninitialized from a scheduling perspective:
    /// access data must be populated before graph building and execution.
    #[inline]
    pub fn new_uninit(system: UnitSystem) -> Self {
        Self {
            system,
            access: AccessTable::new(),
        }
    }
}
