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
use voker_os::sync::atomic::AtomicU32;
use voker_os::utils::SegQueue;
use voker_reflect::Reflect;
use voker_reflect::info::TypePath;

use crate::Asset;

// -----------------------------------------------------------------------------
// AssetIndex

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[derive(Ord, PartialOrd, Reflect, Serialize, Deserialize)]
#[reflect(Clone, Debug, PartialEq, Hash, PartialOrd, Serialize, Deserialize)]
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

// -----------------------------------------------------------------------------
// AssetIndexAllocator

pub(crate) struct AssetIndexAllocator {
    next_index: AtomicU32,
    recycled_queue: SegQueue<AssetIndex>,
    recycled: SegQueue<AssetIndex>,
}

impl AssetIndexAllocator {
    pub(crate) const fn new() -> Self {
        Self {
            next_index: AtomicU32::new(0),
            recycled_queue: SegQueue::new(),
            recycled: SegQueue::new(),
        }
    }
}

impl AssetIndexAllocator {
    pub fn reserve(&self) -> AssetIndex {
        if let Some(mut recycled) = self.recycled_queue.pop() {
            recycled.generation += 1;
            self.recycled.push(recycled);
            recycled
        } else {
            AssetIndex {
                index: self.next_index.fetch_add(1, Ordering::Relaxed),
                generation: 0,
            }
        }
    }

    pub fn recycle(&self, index: AssetIndex) {
        self.recycled_queue.push(index);
    }
}

// -----------------------------------------------------------------------------
// InternalAssetId

#[derive(Debug, Copy, Clone, Hash)]
#[derive(Eq, PartialEq, PartialOrd, Ord)]
enum InternalAssetId {
    Index(AssetIndex),
    Uuid(Uuid),
}

impl From<AssetIndex> for InternalAssetId {
    #[inline(always)]
    fn from(value: AssetIndex) -> Self {
        Self::Index(value)
    }
}

impl From<Uuid> for InternalAssetId {
    #[inline(always)]
    fn from(value: Uuid) -> Self {
        Self::Uuid(value)
    }
}

// -----------------------------------------------------------------------------
// InternalAssetId

#[derive(Debug, Copy, Clone, Reflect)]
#[reflect(Debug, Clone, PartialEq, Hash, PartialOrd)]
pub enum UntypedAssetId {
    Index { type_id: TypeId, index: AssetIndex },
    Uuid { type_id: TypeId, uuid: Uuid },
}

impl UntypedAssetId {
    #[inline]
    pub const fn type_id(&self) -> TypeId {
        match self {
            Self::Index { type_id, .. } | Self::Uuid { type_id, .. } => *type_id,
        }
    }

    #[inline]
    const fn internal(self) -> InternalAssetId {
        match self {
            Self::Index { index, .. } => InternalAssetId::Index(index),
            Self::Uuid { uuid, .. } => InternalAssetId::Uuid(uuid),
        }
    }
}

impl Display for UntypedAssetId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut writer = f.debug_struct("UntypedAssetId");
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

impl PartialEq for UntypedAssetId {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.type_id() == other.type_id() && self.internal().eq(&other.internal())
    }
}

impl Eq for UntypedAssetId {}

impl Hash for UntypedAssetId {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.internal().hash(state);
        self.type_id().hash(state);
    }
}

impl Ord for UntypedAssetId {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.type_id()
            .cmp(&other.type_id())
            .then_with(|| self.internal().cmp(&other.internal()))
    }
}

impl PartialOrd for UntypedAssetId {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// -----------------------------------------------------------------------------
// AssetId

#[derive(Reflect, Serialize, Deserialize)]
#[reflect(Clone, Default, Debug, PartialEq, Hash, Serialize, Deserialize)]
pub enum AssetId<A: Asset> {
    /// A small / efficient runtime identifier that can be used to efficiently look up an asset stored in [`Assets`]. This is
    /// the "default" identifier used for assets. The alternative(s) (ex: [`AssetId::Uuid`]) will only be used if assets are
    /// explicitly registered that way.
    ///
    /// [`Assets`]: crate::Assets
    Index {
        /// The unstable, opaque index of the asset.
        index: AssetIndex,
        /// A marker to store the type information of the asset.
        #[serde(skip)]
        #[reflect(ignore, clone, default)]
        marker: PhantomData<fn() -> A>,
    },
    /// A stable-across-runs / const asset identifier. This will only be used if an asset is explicitly registered in [`Assets`]
    /// with one.
    ///
    /// [`Assets`]: crate::Assets
    Uuid {
        /// The UUID provided during asset registration.
        uuid: Uuid,
    },
}

impl<A: Asset> AssetId<A> {
    /// The uuid for the default [`AssetId`].
    pub const DEFAULT_UUID: Uuid = Uuid::from_u128(200809721996911295814598172825939264631);

    /// This asset id _should_ never be valid.
    pub const INVALID_UUID: Uuid = Uuid::from_u128(108428345662029828789348721013522787528);

    #[inline]
    pub const fn invalid() -> Self {
        Self::Uuid {
            uuid: Self::INVALID_UUID,
        }
    }

    #[inline]
    pub const fn untyped(self) -> UntypedAssetId {
        let type_id = TypeId::of::<A>();
        match self {
            AssetId::Index { index, .. } => UntypedAssetId::Index { type_id, index },
            AssetId::Uuid { uuid } => UntypedAssetId::Uuid { type_id, uuid },
        }
    }

    #[inline]
    const fn internal(self) -> InternalAssetId {
        match self {
            AssetId::Index { index, .. } => InternalAssetId::Index(index),
            AssetId::Uuid { uuid } => InternalAssetId::Uuid(uuid),
        }
    }
}

impl<A: Asset> Default for AssetId<A> {
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
        Debug::fmt(self, f)
    }
}

impl<A: Asset> Debug for AssetId<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AssetId::Index { index, .. } => {
                write!(
                    f,
                    "AssetId<{}>{{ index: {}, generation: {}}}",
                    <A as TypePath>::type_path(),
                    index.index,
                    index.generation
                )
            }
            AssetId::Uuid { uuid } => {
                write!(
                    f,
                    "AssetId<{}>{{uuid: {}}}",
                    <A as TypePath>::type_path(),
                    uuid
                )
            }
        }
    }
}

impl<A: Asset> Hash for AssetId<A> {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.internal().hash(state);
        TypeId::of::<A>().hash(state);
    }
}

impl<A: Asset> PartialEq for AssetId<A> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.internal().eq(&other.internal())
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
        self.internal().cmp(&other.internal())
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
// Conversion

impl<A: Asset> From<AssetId<A>> for UntypedAssetId {
    #[inline]
    fn from(value: AssetId<A>) -> Self {
        value.untyped()
    }
}

impl<A: Asset> TryFrom<UntypedAssetId> for AssetId<A> {
    type Error = AssetIdTypedError;

    #[inline]
    fn try_from(value: UntypedAssetId) -> Result<Self, Self::Error> {
        match value {
            UntypedAssetId::Index { type_id, index } => {
                if type_id == TypeId::of::<A>() {
                    return Ok(AssetId::Index {
                        index,
                        marker: PhantomData,
                    });
                }
                voker_utils::cold_path();
                let expect = TypeId::of::<A>();
                let actual = type_id;
                Err(AssetIdTypedError { expect, actual })
            }
            UntypedAssetId::Uuid { type_id, uuid } => {
                if type_id == TypeId::of::<A>() {
                    return Ok(AssetId::Uuid { uuid });
                }
                voker_utils::cold_path();
                let expect = TypeId::of::<A>();
                let actual = type_id;
                Err(AssetIdTypedError { expect, actual })
            }
        }
    }
}

impl UntypedAssetId {
    #[inline]
    pub const fn typed_unchecked<A: Asset>(self) -> AssetId<A> {
        match self {
            UntypedAssetId::Index { index, .. } => AssetId::Index {
                index,
                marker: PhantomData,
            },
            UntypedAssetId::Uuid { uuid, .. } => AssetId::Uuid { uuid },
        }
    }

    #[inline]
    pub fn typed_debug_checked<A: Asset>(self) -> AssetId<A> {
        debug_assert_eq!(
            self.type_id(),
            TypeId::of::<A>(),
            "The target AssetId<{}>'s TypeId does not match the TypeId of this UntypedAssetId",
            <A as TypePath>::type_path(),
        );
        self.typed_unchecked()
    }

    #[inline]
    pub fn typed<A: Asset>(self) -> AssetId<A> {
        #[cold]
        #[inline(never)]
        fn invalid_type(s: &str) -> ! {
            panic!(
                "The target AssetId<{s}>'s TypeId does not match the TypeId of this UntypedAssetId"
            )
        }

        self.try_typed()
            .unwrap_or_else(|_| invalid_type(<A as TypePath>::type_path()))
    }

    #[inline]
    pub fn try_typed<A: Asset>(self) -> Result<AssetId<A>, AssetIdTypedError> {
        AssetId::try_from(self)
    }
}

/// Errors preventing the conversion of to/from an [`UntypedAssetId`] and an [`AssetId`].
#[derive(Error, Debug, PartialEq, Clone)]
#[error(
    "This UntypedAssetId is for {actual:?} and cannot be converted into an AssetId<{expect:?}>"
)]
pub struct AssetIdTypedError {
    /// The [`TypeId`] that we are trying to convert to.
    expect: TypeId,
    /// The [`TypeId`] that we are trying to convert from.
    actual: TypeId,
}

impl<A: Asset> PartialEq<UntypedAssetId> for AssetId<A> {
    #[inline]
    fn eq(&self, other: &UntypedAssetId) -> bool {
        TypeId::of::<A>() == other.type_id() && self.internal().eq(&other.internal())
    }
}

impl<A: Asset> PartialEq<AssetId<A>> for UntypedAssetId {
    #[inline]
    fn eq(&self, other: &AssetId<A>) -> bool {
        TypeId::of::<A>() == self.type_id() && other.internal().eq(&self.internal())
    }
}

impl<A: Asset> PartialOrd<UntypedAssetId> for AssetId<A> {
    #[inline]
    fn partial_cmp(&self, other: &UntypedAssetId) -> Option<core::cmp::Ordering> {
        if TypeId::of::<A>() != other.type_id() {
            None
        } else {
            Some(self.internal().cmp(&other.internal()))
        }
    }
}

impl<A: Asset> PartialOrd<AssetId<A>> for UntypedAssetId {
    #[inline]
    fn partial_cmp(&self, other: &AssetId<A>) -> Option<core::cmp::Ordering> {
        Some(other.partial_cmp(self)?.reverse())
    }
}

// -----------------------------------------------------------------------------
// AssetSourceId

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
// ErasedAssetIndex

/// An asset index bundled with its (dynamic) type.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub(crate) struct ErasedAssetIndex {
    pub(crate) index: AssetIndex,
    pub(crate) type_id: TypeId,
}

impl ErasedAssetIndex {
    pub(crate) const fn new(index: AssetIndex, type_id: TypeId) -> Self {
        Self { index, type_id }
    }
}

impl Display for ErasedAssetIndex {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ErasedAssetIndex")
            .field("type_id", &self.type_id)
            .field("index", &self.index.index)
            .field("generation", &self.index.generation)
            .finish()
    }
}

#[derive(Error, Debug)]
#[error("Attempted to create a TypedAssetIndex from a Uuid")]
pub(crate) struct UuidNotSupportedError;

impl TryFrom<UntypedAssetId> for ErasedAssetIndex {
    type Error = UuidNotSupportedError;

    fn try_from(asset_id: UntypedAssetId) -> Result<Self, Self::Error> {
        match asset_id {
            UntypedAssetId::Index { type_id, index } => Ok(ErasedAssetIndex { index, type_id }),
            UntypedAssetId::Uuid { .. } => Err(UuidNotSupportedError),
        }
    }
}

impl From<ErasedAssetIndex> for UntypedAssetId {
    fn from(value: ErasedAssetIndex) -> Self {
        Self::Index {
            type_id: value.type_id,
            index: value.index,
        }
    }
}
