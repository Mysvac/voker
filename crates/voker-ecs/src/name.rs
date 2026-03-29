use alloc::borrow::Cow;
use alloc::string::String;
use core::cmp::Ordering;
use core::fmt::{Debug, Display};
use core::hash::{BuildHasher, Hash};
use core::ops::Deref;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use voker_reflect::Reflect;
use voker_utils::hash::FixedHashState;

use crate::component::Component;
use crate::utils::Cloner;

/// A pre-built component for representing names.
///
/// This component stores a name string along with its precomputed hash value,
/// providing efficient comparison and lookup operations. The hash is computed
/// automatically and cached to avoid repeated hashing overhead.
#[derive(Reflect, Clone)]
#[reflect(Opaque, full)]
pub struct Name {
    name: Cow<'static, str>,
    hash: u64,
}

/// Manually implement to accelerate compilation.
impl Component for Name {
    fn cloner() -> Option<Cloner> {
        Some(Cloner::clonable::<Self>())
    }
}

impl Hash for Name {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl PartialEq for Name {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash && self.as_str() == other.as_str()
    }
}

impl Eq for Name {}

impl PartialOrd for Name {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Name {
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(self.as_str(), other.as_str())
    }
}

impl Display for Name {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        Display::fmt(self.as_str(), f)
    }
}

impl Debug for Name {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        Debug::fmt(self.as_str(), f)
    }
}

impl Default for Name {
    fn default() -> Self {
        Name::new("")
    }
}

impl Name {
    fn update_hash(&mut self) {
        self.hash = FixedHashState.hash_one(&self.name);
    }

    /// Creates a new [`Name`] from any string-like type.
    ///
    /// The internal hash will be computed immediately.
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        let name = name.into();
        let mut name = Name { name, hash: 0 };
        name.update_hash();
        name
    }

    /// Sets the entity's name.
    ///
    /// The internal hash will be re-computed.
    #[inline]
    pub fn set(&mut self, name: impl Into<Cow<'static, str>>) {
        *self = Name::new(name);
    }

    /// Updates the name of the entity in place.
    ///
    /// The internal hash will be re-computed.
    #[inline]
    pub fn mutate<F: FnOnce(&mut String)>(&mut self, f: F) {
        f(self.name.to_mut());
        self.update_hash();
    }

    /// Gets the name of the entity as a `&str`.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.name
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        &self.name
    }
}

impl From<&str> for Name {
    fn from(name: &str) -> Self {
        Name::new(String::from(name))
    }
}

impl From<String> for Name {
    fn from(name: String) -> Self {
        Name::new(name)
    }
}

impl From<&Name> for String {
    fn from(val: &Name) -> String {
        String::from(val.as_str())
    }
}

impl From<Name> for String {
    fn from(val: Name) -> String {
        val.name.into_owned()
    }
}

impl Deref for Name {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.name.as_ref()
    }
}

impl Serialize for Name {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Name {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct NameVisitor;

        impl<'de> Visitor<'de> for NameVisitor {
            type Value = Name;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str(core::any::type_name::<Name>())
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(Name::new(String::from(v)))
            }

            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(Name::new(v))
            }
        }

        deserializer.deserialize_str(NameVisitor)
    }
}
