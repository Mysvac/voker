//! System identity — stable unique IDs for each system in the ECS.
//!
//! [`SystemId`] wraps a `TypeId` (derived from the system's concrete type) and
//! an optional [`InternedSystemSet`] tag that disambiguates instances of the
//! same system type placed in different sets (e.g., `SystemSetBegin`/`SystemSetEnd`).

use core::any::TypeId;
use core::fmt::{Debug, Display};
use core::hash::Hash;

use voker_utils::debug::DebugName;

use crate::system::{InternedSystemSet, SystemSet};

// -----------------------------------------------------------------------------
// SystemId

/// A unique identifier for a system.
#[derive(Clone, Copy)]
pub struct SystemId {
    name: DebugName,
    type_id: TypeId,
    in_set: InternedSystemSet,
}

impl PartialEq for SystemId {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id && self.in_set == other.in_set
    }
}

impl Eq for SystemId {}

impl Hash for SystemId {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.type_id.hash(state);
    }
}

impl SystemId {
    #[inline(always)]
    pub fn of<T: 'static>() -> Self {
        Self {
            name: DebugName::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            in_set: ().intern(),
        }
    }

    #[inline(always)]
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    #[inline(always)]
    pub fn name(&self) -> DebugName {
        self.name
    }

    #[inline(always)]
    pub fn system_set(&self) -> InternedSystemSet {
        self.in_set
    }

    #[must_use]
    #[inline(always)]
    pub fn with_system_set(self, set: InternedSystemSet) -> Self {
        Self {
            name: self.name,
            type_id: self.type_id,
            in_set: set,
        }
    }
}

impl Debug for SystemId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}<{:?}>({:?})", self.name, self.type_id, self.in_set)
    }
}

impl Display for SystemId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}({:?})", self.name, self.in_set)
    }
}
