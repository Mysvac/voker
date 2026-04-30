use alloc::string::String;

use core::any::TypeId;
use core::fmt::{Debug, Display};
use core::hash::Hash;
use core::marker::PhantomData;
use core::sync::atomic::Ordering;

use atomicow::CowArc;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use voker_os::atomic::AtomicU32;
use voker_os::utils::SegQueue;
use voker_reflect::Reflect;

use crate::asset::Asset;

// -----------------------------------------------------------------------------
// AssetIndex

/// A generation-aware slot index that uniquely identifies an asset within [`Assets<A>`].
///
/// The index is laid out as two packed `u32` fields (`index` + `generation`) in a
/// single `u64` on both little-endian and big-endian targets, making bitwise
/// conversion via [`to_bits`](AssetIndex::to_bits) / [`from_bits`](AssetIndex::from_bits)
/// safe and endian-independent.
///
/// [`Assets<A>`]: crate::assets::Assets
#[derive(Debug, Copy, Clone, Eq, PartialEq, Reflect)]
#[reflect(Opaque)] // Fields order is not fixed, use `Opaque` to ensure logical stability.
#[reflect(Clone, Debug, Hash, PartialEq, PartialOrd, Serialize, Deserialize)]
#[repr(C, align(8))]
pub struct AssetIndex {
    #[cfg(target_endian = "little")]
    pub(crate) index: u32,
    pub(crate) generation: u32,
    #[cfg(target_endian = "big")]
    pub(crate) index: u32,
}

impl AssetIndex {
    #[inline(always)]
    pub const fn to_bits(self) -> u64 {
        #[expect(unsafe_code, reason = "Stable Conversion")]
        unsafe {
            core::mem::transmute::<Self, u64>(self)
        }
    }

    #[inline(always)]
    pub const fn from_bits(bits: u64) -> Self {
        #[expect(unsafe_code, reason = "Stable Conversion")]
        unsafe {
            core::mem::transmute::<u64, Self>(bits)
        }
    }
}

impl PartialOrd for AssetIndex {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AssetIndex {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.to_bits().cmp(&other.to_bits())
    }
}

impl Hash for AssetIndex {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.to_bits());
    }
}

impl Serialize for AssetIndex {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.to_bits())
    }
}

impl<'de> Deserialize<'de> for AssetIndex {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self::from_bits(Deserialize::deserialize(deserializer)?))
    }
}

// -----------------------------------------------------------------------------
// AssetIndexAllocator

/// Lock-free allocator for [`AssetIndex`] values.
///
/// New indices are handed out by atomically incrementing `next_index`.
/// Freed slots are first pushed onto `recycled_queue`; the next call to
/// [`reserve`](AssetIndexAllocator::reserve) pops from there, bumps the generation,
/// re-pushes the entry to `recycled`, and returns it, making generation-based
/// aliasing detectable.
pub(crate) struct AssetIndexAllocator {
    pub next_index: AtomicU32,
    pub recycled_queue: SegQueue<AssetIndex>,
    pub recycled: SegQueue<AssetIndex>,
}

impl AssetIndexAllocator {
    pub const MAX_ASSET_INDEX: u32 = i32::MAX as u32;

    pub const fn new() -> Self {
        Self {
            next_index: AtomicU32::new(0),
            recycled_queue: SegQueue::new(),
            recycled: SegQueue::new(),
        }
    }

    pub fn reserve(&self) -> AssetIndex {
        #[cold]
        #[inline(never)]
        fn too_many_assets(next_index: &AtomicU32) -> ! {
            next_index.fetch_sub(1, Ordering::Relaxed);
            panic!("too many assets");
        }

        if let Some(mut recycled) = self.recycled_queue.pop() {
            recycled.generation += 1;
            self.recycled.push(recycled);
            return recycled;
        }

        let index = self.next_index.fetch_add(1, Ordering::Relaxed);
        if index <= Self::MAX_ASSET_INDEX {
            return AssetIndex {
                index,
                generation: 0,
            };
        }

        too_many_assets(&self.next_index)
    }

    pub fn recycle(&self, index: AssetIndex) {
        self.recycled_queue.push(index);
    }
}

// -----------------------------------------------------------------------------
// AssetId

/// A stable, typed identifier for a managed asset of type `A`.
///
/// There are two variants:
/// - [`Index`](AssetId::Index): a runtime slot index, valid only within the current session.
/// - [`Uuid`](AssetId::Uuid): a stable UUID, suitable for constant or cross-session references.
///
/// The default value is [`AssetId::Uuid`] with the well-known
/// [`DEFAULT_UUID`](AssetId::DEFAULT_UUID), which is guaranteed to be distinct from
/// [`INVALID_UUID`](AssetId::INVALID_UUID).
#[derive(Reflect, Serialize, Deserialize)]
#[reflect(Clone, Default, Debug, PartialEq, Hash, Serialize, Deserialize)]
pub enum AssetId<A: Asset> {
    Index {
        index: AssetIndex,
        #[serde(skip)]
        #[reflect(ignore, clone, default)]
        marker: PhantomData<fn() -> A>,
    },
    Uuid {
        uuid: Uuid,
    },
}

const INVALID_UUID_D4: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
const DEFAULT_UUID_D4: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];

impl<A: Asset> AssetId<A> {
    /// This asset id _should_ never be valid.
    pub const INVALID_UUID: Uuid =
        Uuid::from_fields(u32::MAX, u16::MAX, u16::MAX, &INVALID_UUID_D4);

    /// The uuid for the default [`AssetId`].
    pub const DEFAULT_UUID: Uuid =
        Uuid::from_fields(u32::MAX, u16::MAX, u16::MAX, &DEFAULT_UUID_D4);

    #[inline]
    pub const fn invalid() -> Self {
        Self::Uuid {
            uuid: Self::INVALID_UUID,
        }
    }

    #[inline]
    pub const fn erased(self) -> ErasedAssetId {
        let type_id = TypeId::of::<A>();
        match self {
            AssetId::Index { index, .. } => ErasedAssetId::Index { type_id, index },
            AssetId::Uuid { uuid } => ErasedAssetId::Uuid { type_id, uuid },
        }
    }
}

impl<A: Asset> Default for AssetId<A> {
    #[inline]
    fn default() -> Self {
        AssetId::Uuid {
            uuid: Self::DEFAULT_UUID,
        }
    }
}

impl<A: Asset> Clone for AssetId<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Asset> Copy for AssetId<A> {}

impl<A: Asset> Display for AssetId<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AssetId::Index { index, .. } => {
                write!(
                    f,
                    "AssetId<{}>{{ index: {}, generation: {}}}",
                    core::any::type_name::<A>(),
                    index.index,
                    index.generation
                )
            }
            AssetId::Uuid { uuid } => {
                write!(
                    f,
                    "AssetId<{}>{{uuid: {}}}",
                    core::any::type_name::<A>(),
                    uuid
                )
            }
        }
    }
}

impl<A: Asset> Debug for AssetId<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(self, f)
    }
}

impl<A: Asset> Hash for AssetId<A> {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        TypeId::of::<A>().hash(state);
        match self {
            AssetId::Index { index, .. } => {
                index.hash(state);
            }
            AssetId::Uuid { uuid } => {
                uuid.hash(state);
            }
        }
    }
}

impl<A: Asset> PartialEq for AssetId<A> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        use AssetId::{Index, Uuid};
        match (self, other) {
            (Index { index: x, .. }, Index { index: y, .. }) => *x == *y,
            (Uuid { uuid: x }, Uuid { uuid: y }) => *x == *y,
            _ => false,
        }
    }
}

impl<A: Asset> Eq for AssetId<A> {}

impl<A: Asset> PartialOrd for AssetId<A> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<A: Asset> Ord for AssetId<A> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        use AssetId::{Index, Uuid};
        match (self, other) {
            (Index { index: x, .. }, Index { index: y, .. }) => x.cmp(y),
            (Uuid { uuid: x }, Uuid { uuid: y }) => x.cmp(y),
            (Index { .. }, Uuid { .. }) => core::cmp::Ordering::Less,
            (Uuid { .. }, Index { .. }) => core::cmp::Ordering::Greater,
        }
    }
}

impl<A: Asset> From<AssetIndex> for AssetId<A> {
    #[inline]
    fn from(value: AssetIndex) -> Self {
        Self::Index {
            index: value,
            marker: PhantomData,
        }
    }
}

impl<A: Asset> From<Uuid> for AssetId<A> {
    #[inline]
    fn from(value: Uuid) -> Self {
        Self::Uuid { uuid: value }
    }
}

// -----------------------------------------------------------------------------
// ErasedAssetId

/// A type-erased asset identifier that carries the [`TypeId`] of the concrete asset.
///
/// Equivalent to [`AssetId<A>`] but usable when `A` is not known statically.
/// Convert to/from typed form with [`typed`](ErasedAssetId::typed) or
/// [`try_typed`](ErasedAssetId::try_typed).
#[derive(Copy, Clone, Reflect)]
#[reflect(Debug, Clone, PartialEq, Hash, PartialOrd)]
pub enum ErasedAssetId {
    Index { type_id: TypeId, index: AssetIndex },
    Uuid { type_id: TypeId, uuid: Uuid },
}

impl ErasedAssetId {
    #[inline]
    pub const fn type_id(&self) -> TypeId {
        match self {
            Self::Index { type_id, .. } | Self::Uuid { type_id, .. } => *type_id,
        }
    }
}

impl Hash for ErasedAssetId {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        match self {
            ErasedAssetId::Index { type_id, index } => {
                type_id.hash(state);
                index.hash(state);
            }
            ErasedAssetId::Uuid { type_id, uuid } => {
                type_id.hash(state);
                uuid.hash(state);
            }
        }
    }
}

impl PartialEq for ErasedAssetId {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        use ErasedAssetId::{Index, Uuid};
        match (self, other) {
            (
                Uuid {
                    type_id: t1,
                    uuid: u1,
                },
                Uuid {
                    type_id: t2,
                    uuid: u2,
                },
            ) => *t1 == *t2 && *u1 == *u2,
            (
                Index {
                    type_id: t1,
                    index: i1,
                },
                Index {
                    type_id: t2,
                    index: i2,
                },
            ) => *t1 == *t2 && *i1 == *i2,
            _ => false,
        }
    }
}

impl Eq for ErasedAssetId {}

impl Ord for ErasedAssetId {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        use ErasedAssetId::{Index, Uuid};
        match (self, other) {
            (
                Index {
                    type_id: t1,
                    index: i1,
                },
                Index {
                    type_id: t2,
                    index: i2,
                },
            ) => t1.cmp(t2).then_with(|| i1.cmp(i2)),
            (
                Uuid {
                    type_id: t1,
                    uuid: u1,
                },
                Uuid {
                    type_id: t2,
                    uuid: u2,
                },
            ) => t1.cmp(t2).then_with(|| u1.cmp(u2)),
            (Index { .. }, Uuid { .. }) => core::cmp::Ordering::Less,
            (Uuid { .. }, Index { .. }) => core::cmp::Ordering::Greater,
        }
    }
}

impl PartialOrd for ErasedAssetId {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Display for ErasedAssetId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut writer = f.debug_struct("ErasedAssetId");
        match self {
            Self::Index { index, type_id } => {
                writer
                    .field("type_id", type_id)
                    .field("index", &index.index)
                    .field("generation", &index.generation);
            }
            Self::Uuid { uuid, type_id } => {
                writer.field("type_id", type_id).field("uuid", uuid);
            }
        }
        writer.finish()
    }
}

impl Debug for ErasedAssetId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(self, f)
    }
}

// -----------------------------------------------------------------------------
// ErasedAssetId & AssetId - Conversion

/// Errors preventing the conversion of to/from an [`ErasedAssetId`] and an [`AssetId`].
#[derive(Error, Debug, PartialEq, Clone)]
#[error("ErasedAssetId({actual:?}) cannot be converted into AssetId<{type_name}>({expect:?})")]
pub struct AssetIdTypedError {
    /// The [`TypePath`] that we are trying to convert to.
    type_name: &'static str,
    /// The [`TypeId`] that we are trying to convert to.
    expect: TypeId,
    /// The [`TypeId`] that we are trying to convert from.
    actual: TypeId,
}

impl<A: Asset> From<AssetId<A>> for ErasedAssetId {
    #[inline]
    fn from(value: AssetId<A>) -> Self {
        value.erased()
    }
}

impl<A: Asset> TryFrom<ErasedAssetId> for AssetId<A> {
    type Error = AssetIdTypedError;

    fn try_from(value: ErasedAssetId) -> Result<Self, Self::Error> {
        match value {
            ErasedAssetId::Index { type_id, index } => {
                if type_id == TypeId::of::<A>() {
                    return Ok(AssetId::Index {
                        index,
                        marker: PhantomData,
                    });
                }
                core::hint::cold_path();
                let type_name = core::any::type_name::<A>();
                let expect = TypeId::of::<A>();
                let actual = type_id;
                Err(AssetIdTypedError {
                    type_name,
                    expect,
                    actual,
                })
            }
            ErasedAssetId::Uuid { type_id, uuid } => {
                if type_id == TypeId::of::<A>() {
                    return Ok(AssetId::Uuid { uuid });
                }
                core::hint::cold_path();
                let type_name = core::any::type_name::<A>();
                let expect = TypeId::of::<A>();
                let actual = type_id;
                Err(AssetIdTypedError {
                    type_name,
                    expect,
                    actual,
                })
            }
        }
    }
}

impl ErasedAssetId {
    #[inline]
    pub const fn typed_unchecked<A: Asset>(self) -> AssetId<A> {
        match self {
            ErasedAssetId::Index { index, .. } => AssetId::Index {
                index,
                marker: PhantomData,
            },
            ErasedAssetId::Uuid { uuid, .. } => AssetId::Uuid { uuid },
        }
    }

    #[inline]
    pub fn typed_debug_checked<A: Asset>(self) -> AssetId<A> {
        debug_assert_eq!(
            self.type_id(),
            TypeId::of::<A>(),
            "The target AssetId<{}>'s TypeId does not match this ErasedAssetId",
            core::any::type_name::<A>(),
        );
        self.typed_unchecked()
    }

    #[inline]
    pub fn typed<A: Asset>(self) -> AssetId<A> {
        #[cold]
        #[inline(never)]
        fn type_mismachted(name: &'static str) -> ! {
            panic!("The target AssetId<{name}>'s TypeId does not match this ErasedAssetId")
        }

        self.try_typed::<A>()
            .unwrap_or_else(|_| type_mismachted(core::any::type_name::<A>()))
    }

    #[inline]
    pub fn try_typed<A: Asset>(self) -> Result<AssetId<A>, AssetIdTypedError> {
        AssetId::try_from(self)
    }
}

impl<A: Asset> PartialEq<ErasedAssetId> for AssetId<A> {
    #[inline]
    fn eq(&self, other: &ErasedAssetId) -> bool {
        match (self, other) {
            (AssetId::Index { index, .. }, ErasedAssetId::Index { type_id, index: i2 }) => {
                TypeId::of::<A>() == *type_id && *index == *i2
            }
            (AssetId::Uuid { uuid }, ErasedAssetId::Uuid { type_id, uuid: u2 }) => {
                TypeId::of::<A>() == *type_id && *uuid == *u2
            }
            _ => false,
        }
    }
}

impl<A: Asset> PartialEq<AssetId<A>> for ErasedAssetId {
    #[inline]
    fn eq(&self, other: &AssetId<A>) -> bool {
        PartialEq::eq(other, self)
    }
}

impl<A: Asset> PartialOrd<ErasedAssetId> for AssetId<A> {
    #[inline]
    fn partial_cmp(&self, other: &ErasedAssetId) -> Option<core::cmp::Ordering> {
        if TypeId::of::<A>() != other.type_id() {
            return None;
        }

        match (self, other) {
            (AssetId::Index { index, .. }, ErasedAssetId::Index { index: i2, .. }) => {
                Some(index.cmp(i2))
            }
            (AssetId::Uuid { uuid }, ErasedAssetId::Uuid { uuid: u2, .. }) => Some(uuid.cmp(u2)),
            (AssetId::Index { .. }, ErasedAssetId::Uuid { .. }) => Some(core::cmp::Ordering::Less),
            (AssetId::Uuid { .. }, ErasedAssetId::Index { .. }) => {
                Some(core::cmp::Ordering::Greater)
            }
        }
    }
}

impl<A: Asset> PartialOrd<AssetId<A>> for ErasedAssetId {
    #[inline]
    fn partial_cmp(&self, other: &AssetId<A>) -> Option<core::cmp::Ordering> {
        Some(other.partial_cmp(self)?.reverse())
    }
}

// -----------------------------------------------------------------------------
// AssetSourceId

/// Identifies the [`AssetSource`](crate::io::AssetSource) that owns an asset path.
///
/// - [`Default`](AssetSourceId::Default): the unnamed primary source (e.g. `assets/`).
/// - [`Name`](AssetSourceId::Name): a named secondary source registered with the asset server.
#[derive(Default, Clone, Debug, Eq)]
pub enum AssetSourceId<'a> {
    #[default]
    Default,
    Name(CowArc<'a, str>),
}

impl<'a> AssetSourceId<'a> {
    /// Creates a new [`AssetSourceId`]
    pub fn new(source: Option<impl Into<CowArc<'a, str>>>) -> AssetSourceId<'a> {
        match source {
            Some(source) => AssetSourceId::Name(source.into()),
            None => AssetSourceId::Default,
        }
    }

    /// If this is not already an owned / static id, create one.
    /// Otherwise, it will return itself (with a static lifetime).
    pub fn into_owned(self) -> AssetSourceId<'static> {
        match self {
            AssetSourceId::Default => AssetSourceId::Default,
            AssetSourceId::Name(v) => AssetSourceId::Name(v.into_owned()),
        }
    }

    /// Clones into an owned [`AssetSourceId<'static>`].
    /// This is equivalent to `.clone().into_owned()`.
    #[inline]
    pub fn clone_owned(&self) -> AssetSourceId<'static> {
        self.clone().into_owned()
    }

    /// Returns [`None`] if this is [`AssetSourceId::Default`] and
    /// [`Some`] containing the name if this is [`AssetSourceId::Name`].
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            AssetSourceId::Default => None,
            AssetSourceId::Name(v) => Some(v),
        }
    }
}

impl<'a> Hash for AssetSourceId<'a> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl<'a> PartialEq for AssetSourceId<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str().eq(&other.as_str())
    }
}

impl<'a> Display for AssetSourceId<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.as_str() {
            None => write!(f, "AssetSourceId::Default"),
            Some(v) => write!(f, "AssetSourceId::Name({v})"),
        }
    }
}

impl<'a, 'b> From<&'a AssetSourceId<'b>> for AssetSourceId<'b> {
    fn from(value: &'a AssetSourceId<'b>) -> Self {
        value.clone()
    }
}

// This is only implemented for static lifetimes to ensure `Path::clone`
// does not allocate by ensuring that this is stored as a `CowArc::Static`.
impl From<&'static str> for AssetSourceId<'static> {
    #[inline]
    fn from(value: &'static str) -> Self {
        AssetSourceId::Name(CowArc::Static(value))
    }
}

impl From<String> for AssetSourceId<'static> {
    fn from(value: String) -> Self {
        AssetSourceId::Name(value.into())
    }
}

impl From<Option<&'static str>> for AssetSourceId<'static> {
    fn from(value: Option<&'static str>) -> Self {
        match value {
            Some(value) => AssetSourceId::Name(value.into()),
            None => AssetSourceId::Default,
        }
    }
}

// -----------------------------------------------------------------------------
// TypedAssetIndex

/// An asset index bundled with its (dynamic) type.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct TypedAssetIndex {
    pub index: AssetIndex,
    pub type_id: TypeId,
}

impl TypedAssetIndex {
    #[inline(always)]
    pub const fn new(index: AssetIndex, type_id: TypeId) -> Self {
        Self { index, type_id }
    }
}

impl Display for TypedAssetIndex {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TypedAssetIndex")
            .field("type_id", &self.type_id)
            .field("index", &self.index.index)
            .field("generation", &self.index.generation)
            .finish()
    }
}

#[derive(Error, Debug)]
#[error("Attempted to create a TypedAssetIndex from a Uuid({0})")]
pub struct UuidNotSupportedError(pub(crate) Uuid);

impl TryFrom<ErasedAssetId> for TypedAssetIndex {
    type Error = UuidNotSupportedError;

    #[inline]
    fn try_from(asset_id: ErasedAssetId) -> Result<Self, Self::Error> {
        match asset_id {
            ErasedAssetId::Index { type_id, index } => Ok(TypedAssetIndex { index, type_id }),
            ErasedAssetId::Uuid { uuid, .. } => Err(UuidNotSupportedError(uuid)),
        }
    }
}

impl From<TypedAssetIndex> for ErasedAssetId {
    fn from(value: TypedAssetIndex) -> Self {
        Self::Index {
            type_id: value.type_id,
            index: value.index,
        }
    }
}
