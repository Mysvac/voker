use core::cmp::Ordering;
use core::fmt::{Debug, Display};
use core::hash::Hash;
use core::mem;
use core::num::NonZeroU32;

use serde::{Deserialize, Serialize};
use voker_reflect::derive::Reflect;

// -----------------------------------------------------------------------------
// EntityId

/// This represents the index of an [`Entity`] within the [`Entities`] array.
///
/// This is a unique identifier for an entity in the world, a lighter weight
/// version of [`Entity`].
///
/// This differs from [`Entity`] in that [`Entity`] is unique for all entities
/// total (unless the [`EntityGeneration`] wraps), but this is only unique for
/// entities that are active.
///
/// The valid range is `1..u32::MAX`, not including `u32::MAX`.
///
/// [`Entity`]: crate::entity::Entity
/// [`Entities`]: crate::entity::Entities
#[derive(Clone, Copy, PartialOrd, Ord)]
#[repr(transparent)]
pub struct EntityId(NonZeroU32);

impl EntityId {
    const _STATIC_ASSERT_: () = const {
        const VAL: u32 = 20260101;
        const ID: EntityId = EntityId(NonZeroU32::new(VAL).unwrap());
        assert!(VAL == ID.to_bits());
    };

    /// Gets the index of the entity.
    #[inline(always)]
    const fn to_bits(self) -> u32 {
        unsafe { mem::transmute::<EntityId, u32>(self) }
    }

    /// Gets the index of the entity.
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.to_bits() as usize
    }
}

impl PartialEq for EntityId {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.to_bits() == other.to_bits()
    }
}

impl Eq for EntityId {}

impl Hash for EntityId {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u32(self.to_bits());
    }
}

impl Debug for EntityId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.to_bits(), f)
    }
}

impl Display for EntityId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.to_bits(), f)
    }
}

// -----------------------------------------------------------------------------
// EntityGeneration

/// This tracks different versions or generations of an [`EntityId`].
///
/// Importantly, this can wrap, meaning each generation is not necessarily
/// unique per [`EntityId`].
///
/// # Aliasing
///
/// Internally [`EntityGeneration`] wraps a `u32`, so it can't represent *every*
/// possible generation. Eventually, generations can (and do) wrap or alias.
///
/// This can cause [`Entity`] and [`EntityGeneration`] values to be equal while
/// still referring to different conceptual entities. Therefore, users should not
/// hold an `Entity` for a long time.
///
/// [`Entity`]: crate::entity::Entity
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct EntityGeneration(u32);

impl EntityGeneration {
    /// Represents the first generation of an [`EntityId`].
    pub(crate) const FIRST: Self = Self(0);

    /// Non-wrapping difference between two generations after which a
    /// signed interpretation becomes negative.
    const DIFF_MAX: u32 = 1u32 << 31;

    /// Returns the [`EntityGeneration`] that would result from this many
    /// more `versions` of the corresponding [`EntityId`] from passing.
    #[inline(always)]
    pub const fn wrapping_add(self, versions: u32) -> Self {
        Self(self.0.wrapping_add(versions))
    }

    /// Identical to [`wrapping_add`](Self::wrapping_add) but also returns a `bool` indicating if
    /// after these `versions`, one such version could conflict with a previous one.
    ///
    /// If this happens, this will no longer uniquely identify a version of an
    /// [`EntityId`]. This is called entity aliasing.
    #[inline]
    pub const fn checked_add(self, versions: u32) -> (Self, bool) {
        let raw = self.0.overflowing_add(versions);
        (Self(raw.0), raw.1)
    }
}

impl PartialOrd for EntityGeneration {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for EntityGeneration {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.0.wrapping_sub(other.0) {
            0 => Ordering::Equal,
            1..Self::DIFF_MAX => Ordering::Greater,
            _ => Ordering::Less,
        }
    }
}

impl Hash for EntityGeneration {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u32(self.0);
    }
}

impl Debug for EntityGeneration {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for EntityGeneration {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

// -----------------------------------------------------------------------------
// Entity

/// A unique identifier for an entity, consisting of an ID and generation.
///
/// Entities are frequently created and destroyed, requiring efficient reuse
/// of identifiers. The `id` field represents the entity's index, while
/// `generation` tracks versioning to distinguish between different occupants
/// of the same index, preventing stale references from accessing new data.
///
/// # Memory Layout
///
/// The struct is guaranteed to have the same representation as a `u64`
/// (8-byte aligned) to enable efficient bitwise operations and serialization.
/// Endianness-aware field ordering ensures consistent behavior across platforms.
#[derive(Reflect, Clone, Copy)]
#[reflect(Opaque, serde, clone, hash, eq, cmp, debug)]
#[repr(C, align(8))]
pub struct Entity {
    // Field ordering is endianness-dependent to ensure consistent u64 representation
    #[cfg(target_endian = "little")]
    id: EntityId,
    generation: EntityGeneration,
    #[cfg(target_endian = "big")]
    id: EntityId,
}

impl Entity {
    const _STATIC_ASSERT_: () = const {
        // Ensure `EntityId` is storaged in lower bits.
        assert!(Entity::from_bits(20260101).id.to_bits() == 20260101);
    };

    /// A placeholder entity representing an invalid or uninitialized entity.
    pub const PLACEHOLDER: Self = unsafe { mem::transmute::<u64, Entity>(u64::MAX) };

    /// Creates a new `Entity` from its constituent parts.
    #[inline(always)]
    pub const fn new(id: EntityId, generation: EntityGeneration) -> Entity {
        Self { id, generation }
    }

    /// Creates an `Entity` with the given ID and the first generation.
    #[inline(always)]
    pub const fn from_id(id: EntityId) -> Entity {
        Self {
            id,
            generation: EntityGeneration::FIRST,
        }
    }

    /// Returns the entity's index as a `usize`.
    ///
    /// As same as [`EntityId::index`] .
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.id.index()
    }

    /// Returns the `EntityId` of this entity.
    #[inline(always)]
    pub const fn id(self) -> EntityId {
        self.id
    }

    /// Returns the `EntityGeneration` of this entity.
    #[inline(always)]
    pub const fn generation(self) -> EntityGeneration {
        self.generation
    }

    /// Converts the entity to its raw `u64` representation.
    ///
    /// This is a zero-cost conversion that preserves the exact bit pattern.
    /// Useful for serialization, hashing, and FFI operations.
    #[inline(always)]
    pub const fn to_bits(self) -> u64 {
        unsafe { mem::transmute::<Entity, u64>(self) }
    }

    /// Creates an `Entity` from its raw `u64` representation.
    ///
    /// This is the inverse of [`Entity::to_bits`].
    ///
    /// # Panics
    ///
    /// Panics if the decoded `EntityId` is zero, as zero is reserved for
    /// representing invalid entities.
    #[inline(always)]
    pub const fn from_bits(bits: u64) -> Self {
        unsafe {
            let entity = mem::transmute::<u64, Entity>(bits);
            assert!(mem::transmute::<EntityId, u32>(entity.id) != 0);
            entity
        }
    }

    /// Creates an `Entity` from its raw `u64` representation without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// 1. The decoded `EntityId` is not zero (zero is reserved for invalid entities)
    /// 2. The `bits` parameter was obtained from a valid `Entity::to_bits()` call
    ///    or carefully constructed to match the memory layout
    ///
    /// Violating these conditions may lead to undefined behavior or logical errors.
    #[inline(always)]
    pub const unsafe fn from_bits_unchecked(bits: u64) -> Self {
        // SAFETY: The caller must guarantee the preconditions
        unsafe { mem::transmute::<u64, Entity>(bits) }
    }
}

impl PartialEq for Entity {
    #[inline(always)]
    fn eq(&self, other: &Entity) -> bool {
        self.to_bits() == other.to_bits()
    }
}

impl Eq for Entity {}

impl PartialOrd for Entity {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entity {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_bits().cmp(&other.to_bits())
    }
}

impl Hash for Entity {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.to_bits());
    }
}

impl Debug for Entity {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for Entity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if *self == Self::PLACEHOLDER {
            f.pad("PLACEHOLDER")
        } else {
            f.pad(&alloc::format!("{}v{}", self.index(), self.generation()))
        }
    }
}

impl Serialize for Entity {
    #[inline(always)]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.to_bits())
    }
}

impl<'de> Deserialize<'de> for Entity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let bits: u64 = Deserialize::deserialize(deserializer)?;

        unsafe {
            let entity = mem::transmute::<u64, Entity>(bits);
            if mem::transmute::<EntityId, u32>(entity.id) != 0 {
                return Ok(entity);
            }
            Err(Error::custom(
                "Attempting to deserialize an invalid entity.",
            ))
        }
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::Entity;

    #[test]
    fn entity_is_u64() {
        assert_eq!(
            Entity::from_bits(123456789012_u64).to_bits(),
            123456789012_u64
        );
    }

    #[test]
    fn entity_eq() {
        assert_eq!(Entity::from_bits(12345), Entity::from_bits(12345));
        assert_ne!(Entity::from_bits(12345), Entity::from_bits(54321));
    }
}
