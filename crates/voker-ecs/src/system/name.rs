use alloc::string::String;
use core::fmt::{Debug, Display};
use core::hash::{BuildHasher, Hash};
use core::ops::Deref;

use voker_utils::hash::FixedHashState;

// -----------------------------------------------------------------------------
// SystemName

/// A unique identifier for a system.
///
/// `SystemName` provides an efficient, hashable representation of system names
/// for use in debugging, scheduling, and system identification. It precomputes
/// a hash using a fixed seed, making it suitable for use in hash maps and sets
/// without runtime hashing overhead.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SystemName {
    name: &'static str,
    hash: u64,
}

impl Hash for SystemName {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl SystemName {
    /// Creates a new `SystemName` from a static string.
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            hash: FixedHashState.hash_one(name),
        }
    }

    /// Returns the underlying static string slice.
    pub fn as_str(&self) -> &'static str {
        self.name
    }
}

impl From<&'static str> for SystemName {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

impl From<SystemName> for &'static str {
    fn from(value: SystemName) -> Self {
        value.name
    }
}

impl From<SystemName> for String {
    fn from(value: SystemName) -> Self {
        String::from(value.name)
    }
}

impl Debug for SystemName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name)
    }
}

impl Display for SystemName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name)
    }
}

impl Deref for SystemName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.name
    }
}

impl AsRef<str> for SystemName {
    fn as_ref(&self) -> &str {
        self.name
    }
}
