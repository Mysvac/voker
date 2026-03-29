use core::any::TypeId;
use core::fmt::{Debug, Display};
use core::hash::Hash;

use crate::utils::DebugName;

// -----------------------------------------------------------------------------
// SystemId

/// A unique identifier for a system.
#[derive(Clone, Copy)]
pub struct SystemId {
    name: DebugName,
    type_id: TypeId,
}

impl PartialEq for SystemId {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id
    }
}

impl Eq for SystemId {}

impl PartialOrd for SystemId {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SystemId {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.type_id.cmp(&other.type_id)
    }
}

impl Hash for SystemId {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.type_id.hash(state);
    }
}

impl SystemId {
    pub const fn of<T: 'static>() -> Self {
        Self {
            name: DebugName::type_name::<T>(),
            type_id: TypeId::of::<T>(),
        }
    }

    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    pub const fn debug_name(&self) -> DebugName {
        self.name
    }
}

impl Debug for SystemId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}({:?})", self.name, self.type_id)
    }
}

impl Display for SystemId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}({:?})", self.name, self.type_id)
    }
}
