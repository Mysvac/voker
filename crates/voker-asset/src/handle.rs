use core::any::TypeId;
use core::fmt::Debug;
use core::hash::Hash;
use core::marker::PhantomData;

use crossbeam_channel::{Receiver, Sender};
use thiserror::Error;
use uuid::Uuid;
use voker_ecs::error::GameError;
use voker_os::sync::Arc;
use voker_reflect::Reflect;
use voker_reflect::info::TypePath;
use voker_utils::hash::Equivalent;

use crate::asset::Asset;
use crate::ident::{
    AssetId, AssetIndex, AssetIndexAllocator, ErasedAssetId, TypedAssetIndex, UuidNotSupportedError,
};
use crate::meta::MetaTransform;
use crate::path::AssetPath;

// -----------------------------------------------------------------------------
// DropEvent

#[derive(Debug)]
pub(crate) struct DropEvent {
    pub index: TypedAssetIndex,
    pub asset_server_managed: bool,
}

// -----------------------------------------------------------------------------
// AssetHandleProvider

/// Provides [`Handle`] and [`ErasedHandle`] for a **specific** asset type.
///
/// This should **only** be used for one specific asset type.
#[derive(Clone)]
pub struct AssetHandleProvider {
    pub(crate) allocator: Arc<AssetIndexAllocator>,
    pub(crate) drop_sender: Sender<DropEvent>,
    pub(crate) drop_receiver: Receiver<DropEvent>,
    pub(crate) type_id: TypeId,
}

impl AssetHandleProvider {
    pub(crate) fn new(type_id: TypeId, allocator: Arc<AssetIndexAllocator>) -> Self {
        let (drop_sender, drop_receiver) = crossbeam_channel::unbounded();
        Self {
            type_id,
            allocator,
            drop_sender,
            drop_receiver,
        }
    }

    pub fn reserve_handle(&self) -> ErasedHandle {
        let index = self.allocator.reserve();
        ErasedHandle::Strong(Arc::new(StrongHandle {
            index,
            type_id: self.type_id,
            drop_sender: self.drop_sender.clone(),
            asset_server_managed: false,
            meta_transform: None,
            path: None,
        }))
    }

    pub(crate) fn build_handle(
        &self,
        index: AssetIndex,
        asset_server_managed: bool,
        path: Option<AssetPath<'static>>,
        meta_transform: Option<MetaTransform>,
    ) -> Arc<StrongHandle> {
        Arc::new(StrongHandle {
            index,
            type_id: self.type_id,
            drop_sender: self.drop_sender.clone(),
            meta_transform,
            path,
            asset_server_managed,
        })
    }

    pub(crate) fn alloc_handle(
        &self,
        asset_server_managed: bool,
        path: Option<AssetPath<'static>>,
        meta_transform: Option<MetaTransform>,
    ) -> Arc<StrongHandle> {
        let index = self.allocator.reserve();
        Arc::new(StrongHandle {
            index,
            type_id: self.type_id,
            drop_sender: self.drop_sender.clone(),
            meta_transform,
            path,
            asset_server_managed,
        })
    }
}

// -----------------------------------------------------------------------------
// StrongHandle

#[derive(TypePath)]
#[type_path = "voker_asset::handle::StrongHandle"]
pub struct StrongHandle {
    pub(crate) index: AssetIndex,
    pub(crate) type_id: TypeId,
    pub(crate) asset_server_managed: bool,
    pub(crate) path: Option<AssetPath<'static>>,
    /// Modifies asset meta. This is stored on the handle because it is:
    /// 1. configuration tied to the lifetime of a specific asset load
    /// 2. configuration that must be repeatable when the asset is hot-reloaded
    pub(crate) meta_transform: Option<MetaTransform>,
    pub(crate) drop_sender: Sender<DropEvent>,
}

impl Drop for StrongHandle {
    fn drop(&mut self) {
        let _ = self.drop_sender.send(DropEvent {
            index: TypedAssetIndex {
                index: self.index,
                type_id: self.type_id,
            },
            asset_server_managed: self.asset_server_managed,
        });
    }
}

impl Debug for StrongHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StrongHandle")
            .field("index", &self.index)
            .field("type_id", &self.type_id)
            .field("path", &self.path)
            .field("asset_server_managed", &self.asset_server_managed)
            .finish()
    }
}

// -----------------------------------------------------------------------------
// Handle

#[derive(Reflect)]
#[reflect(Default, Debug, Hash, PartialEq, PartialOrd, Clone)]
pub enum Handle<A: Asset> {
    /// A "strong" reference to a live (or loading) [`Asset`].
    ///
    /// If a [`Handle`] is [`Handle::Strong`], the [`Asset`] will be kept
    /// alive until the [`Handle`] is dropped. Strong handles also provide
    /// access to additional asset metadata.
    Strong(Arc<StrongHandle>),
    /// A reference to an [`Asset`] using a stable-across-runs / const identifier.
    ///
    /// Dropping this handle will not result in the asset being dropped.
    Uuid(
        Uuid,
        #[reflect(ignore, clone, default)] PhantomData<fn() -> A>,
    ),
}

impl<A: Asset> Handle<A> {
    /// Returns the [`AssetId`] of this [`Asset`].
    #[inline]
    pub fn id(&self) -> AssetId<A> {
        match self {
            Handle::Strong(handle) => AssetId::Index {
                index: handle.index,
                marker: PhantomData,
            },
            Handle::Uuid(uuid, ..) => AssetId::Uuid { uuid: *uuid },
        }
    }

    /// Returns the path if this is (1) a strong handle and (2) the asset has a path
    #[inline]
    pub fn path(&self) -> Option<&AssetPath<'static>> {
        match self {
            Handle::Strong(handle) => handle.path.as_ref(),
            Handle::Uuid(..) => None,
        }
    }

    /// Returns `true` if this is a uuid handle.
    #[inline]
    pub fn is_uuid(&self) -> bool {
        matches!(self, Handle::Uuid(..))
    }

    /// Returns `true` if this is a strong handle.
    #[inline]
    pub fn is_strong(&self) -> bool {
        matches!(self, Handle::Strong(_))
    }

    /// Converts this [`Handle`] to an "erased" / "generic-less" [`ErasedHandle`].
    ///
    /// which stores the [`Asset`] type information _inside_ [`ErasedHandle`]. This will return
    /// [`ErasedHandle::Strong`] for [`Handle::Strong`] and [`ErasedHandle::Uuid`] for [`Handle::Uuid`].
    #[inline]
    pub fn erased(self) -> ErasedHandle {
        match self {
            Handle::Strong(handle) => ErasedHandle::Strong(handle),
            Handle::Uuid(uuid, ..) => ErasedHandle::Uuid {
                type_id: TypeId::of::<A>(),
                uuid,
            },
        }
    }
}

// ---------------------------------------------------
// Handle - Basic Trait

impl<A: Asset> Debug for Handle<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = <A as TypePath>::type_name();
        match self {
            Handle::Strong(handle) => {
                write!(
                    f,
                    "StrongHandle<{name}>{{ index: {:?}, type_id: {:?}, path: {:?} }}",
                    handle.index, handle.type_id, handle.path
                )
            }
            Handle::Uuid(uuid, ..) => write!(f, "UuidHandle<{name}>({uuid:?})"),
        }
    }
}

impl<A: Asset> Default for Handle<A> {
    fn default() -> Self {
        Handle::Uuid(AssetId::<A>::DEFAULT_UUID, PhantomData)
    }
}

impl<T: Asset> Clone for Handle<T> {
    fn clone(&self) -> Self {
        match self {
            Handle::Strong(handle) => Handle::Strong(handle.clone()),
            Handle::Uuid(uuid, ..) => Handle::Uuid(*uuid, PhantomData),
        }
    }
}

impl<A: Asset> Hash for Handle<A> {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}

impl<A: Asset> PartialOrd for Handle<A> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<A: Asset> Ord for Handle<A> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id().cmp(&other.id())
    }
}

impl<A: Asset> PartialEq for Handle<A> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl<A: Asset> Eq for Handle<A> {}

// ---------------------------------------------------
// Handle & AssetId

impl<A: Asset> From<Uuid> for Handle<A> {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Handle::Uuid(uuid, PhantomData)
    }
}

impl<T: Asset> Equivalent<Handle<T>> for AssetId<T> {
    fn equivalent(&self, key: &Handle<T>) -> bool {
        *self == key.id()
    }
}

impl<A: Asset> From<&Handle<A>> for AssetId<A> {
    #[inline]
    fn from(value: &Handle<A>) -> Self {
        value.id()
    }
}

impl<A: Asset> From<&Handle<A>> for ErasedAssetId {
    #[inline]
    fn from(value: &Handle<A>) -> Self {
        value.id().into()
    }
}

impl<A: Asset> From<&mut Handle<A>> for AssetId<A> {
    #[inline]
    fn from(value: &mut Handle<A>) -> Self {
        value.id()
    }
}

impl<A: Asset> From<&mut Handle<A>> for ErasedAssetId {
    #[inline]
    fn from(value: &mut Handle<A>) -> Self {
        value.id().into()
    }
}

// -----------------------------------------------------------------------------
// ErasedHandle

#[derive(Clone, Reflect)]
#[reflect(PartialOrd, PartialEq, Hash, Clone, Debug)]
pub enum ErasedHandle {
    /// A strong handle, which will keep the referenced [`Asset`]
    /// alive until all strong handles are dropped.
    Strong(Arc<StrongHandle>),
    /// A UUID handle, which does not keep the referenced [`Asset`] alive.
    Uuid {
        /// An identifier that records the underlying asset type.
        type_id: TypeId,
        /// The UUID provided during asset registration.
        uuid: Uuid,
    },
}

impl ErasedHandle {
    /// Returns the equivalent of [`Handle`]'s default implementation for the given type ID.
    #[inline]
    pub const fn default_for_type(type_id: TypeId) -> Self {
        Self::Uuid {
            type_id,
            uuid: AssetId::<()>::DEFAULT_UUID,
        }
    }

    /// Returns the [`ErasedAssetId`] for the referenced asset.
    #[inline]
    pub fn id(&self) -> ErasedAssetId {
        match self {
            ErasedHandle::Strong(handle) => ErasedAssetId::Index {
                type_id: handle.type_id,
                index: handle.index,
            },
            ErasedHandle::Uuid { type_id, uuid } => ErasedAssetId::Uuid {
                uuid: *uuid,
                type_id: *type_id,
            },
        }
    }

    /// Returns the path if this is (1) a strong handle and (2) the asset has a path
    #[inline]
    pub fn path(&self) -> Option<&AssetPath<'static>> {
        match self {
            ErasedHandle::Strong(handle) => handle.path.as_ref(),
            ErasedHandle::Uuid { .. } => None,
        }
    }

    /// Returns the [`TypeId`] of the referenced [`Asset`].
    #[inline]
    pub fn type_id(&self) -> TypeId {
        match self {
            ErasedHandle::Strong(handle) => handle.type_id,
            ErasedHandle::Uuid { type_id, .. } => *type_id,
        }
    }

    /// The "meta transform" for the strong handle.
    ///
    /// This will only be [`Some`] if the handle is strong and there is
    /// a meta transform associated with it.
    #[inline]
    pub fn meta_transform(&self) -> Option<&MetaTransform> {
        match self {
            ErasedHandle::Strong(handle) => handle.meta_transform.as_ref(),
            ErasedHandle::Uuid { .. } => None,
        }
    }
}

impl PartialEq for ErasedHandle {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id() && self.type_id() == other.type_id()
    }
}

impl Eq for ErasedHandle {}

impl PartialOrd for ErasedHandle {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        if self.type_id() == other.type_id() {
            self.id().partial_cmp(&other.id())
        } else {
            None
        }
    }
}

impl Hash for ErasedHandle {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}

impl Debug for ErasedHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ErasedHandle::Strong(handle) => {
                write!(
                    f,
                    "StrongHandle{{ type_id: {:?}, id: {:?}, path: {:?} }}",
                    handle.type_id, handle.index, handle.path
                )
            }
            ErasedHandle::Uuid { type_id, uuid } => {
                write!(f, "UuidHandle{{ type_id: {type_id:?}, uuid: {uuid:?} }}",)
            }
        }
    }
}

impl From<&ErasedHandle> for ErasedAssetId {
    #[inline]
    fn from(value: &ErasedHandle) -> Self {
        value.id()
    }
}

impl From<&mut ErasedHandle> for ErasedAssetId {
    #[inline]
    fn from(value: &mut ErasedHandle) -> Self {
        value.id()
    }
}

// -----------------------------------------------------------------------------
// ErasedHandle & Handle - Conversion

/// Errors preventing the conversion of to/from an [`ErasedHandle`] and an [`Handle`].
#[derive(GameError, Error, Debug, PartialEq, Clone)]
#[game_error(severity = "error")]
#[error(
    "This ErasedHandle({actual:?}) cannot be converted into an Handle<{expect_path}>({expect:?})"
)]
pub struct AssetHandleTypedError {
    /// The [`TypePath`] that we are trying to convert to.
    expect_path: &'static str,
    /// The [`TypeId`] that we are trying to convert to.
    expect: TypeId,
    /// The [`TypeId`] that we are trying to convert from.
    actual: TypeId,
}

impl<A: Asset> From<Handle<A>> for ErasedHandle {
    #[inline]
    fn from(value: Handle<A>) -> Self {
        value.erased()
    }
}

impl<A: Asset> TryFrom<ErasedHandle> for Handle<A> {
    type Error = AssetHandleTypedError;

    fn try_from(value: ErasedHandle) -> Result<Self, Self::Error> {
        match value {
            ErasedHandle::Strong(handle) => {
                if handle.type_id == TypeId::of::<A>() {
                    return Ok(Handle::Strong(handle));
                }

                voker_utils::cold_path();
                let expect_path = <A as TypePath>::type_path();
                let expect = TypeId::of::<A>();
                let actual = handle.type_id;
                Err(AssetHandleTypedError {
                    expect_path,
                    expect,
                    actual,
                })
            }
            ErasedHandle::Uuid { type_id, uuid } => {
                if type_id == TypeId::of::<A>() {
                    return Ok(Handle::Uuid(uuid, PhantomData));
                }

                voker_utils::cold_path();
                let expect_path = <A as TypePath>::type_path();
                let expect = TypeId::of::<A>();
                let actual = type_id;
                Err(AssetHandleTypedError {
                    expect_path,
                    expect,
                    actual,
                })
            }
        }
    }
}

impl ErasedHandle {
    /// Converts to a typed Handle. This _will not check if the target Handle type matches_.
    #[inline]
    pub fn typed_unchecked<A: Asset>(self) -> Handle<A> {
        match self {
            ErasedHandle::Strong(handle) => Handle::Strong(handle),
            ErasedHandle::Uuid { uuid, .. } => Handle::Uuid(uuid, PhantomData),
        }
    }

    /// Converts to a typed Handle. This will check the type when compiled with debug asserts.
    #[inline]
    pub fn typed_debug_checked<A: Asset>(self) -> Handle<A> {
        debug_assert_eq!(
            self.type_id(),
            TypeId::of::<A>(),
            "The target Handle<A>'s TypeId does not match the TypeId of this ErasedHandle"
        );
        self.typed_unchecked()
    }

    /// Converts to a typed Handle. This will panic if the internal [`TypeId`] does not match the given asset type `A`
    #[inline]
    pub fn typed<A: Asset>(self) -> Handle<A> {
        #[cold]
        #[inline(never)]
        fn type_mismachted(path: fn() -> &'static str) -> ! {
            let name = path();
            panic!("The target Handle<{name}>'s TypeId does not match this ErasedHandle")
        }

        self.try_typed::<A>()
            .unwrap_or_else(|_| type_mismachted(<A as TypePath>::type_path))
    }

    #[inline]
    pub fn try_typed<A: Asset>(self) -> Result<Handle<A>, AssetHandleTypedError> {
        Handle::try_from(self)
    }
}

impl<A: Asset> PartialEq<ErasedHandle> for Handle<A> {
    #[inline]
    fn eq(&self, other: &ErasedHandle) -> bool {
        TypeId::of::<A>() == other.type_id() && self.id() == other.id()
    }
}

impl<A: Asset> PartialEq<Handle<A>> for ErasedHandle {
    #[inline]
    fn eq(&self, other: &Handle<A>) -> bool {
        TypeId::of::<A>() == self.type_id() && other.id() == self.id()
    }
}

impl<A: Asset> PartialOrd<ErasedHandle> for Handle<A> {
    #[inline]
    fn partial_cmp(&self, other: &ErasedHandle) -> Option<core::cmp::Ordering> {
        if TypeId::of::<A>() != other.type_id() {
            None
        } else {
            self.id().partial_cmp(&other.id())
        }
    }
}

impl<A: Asset> PartialOrd<Handle<A>> for ErasedHandle {
    #[inline]
    fn partial_cmp(&self, other: &Handle<A>) -> Option<core::cmp::Ordering> {
        if TypeId::of::<A>() != self.type_id() {
            None
        } else {
            self.id().partial_cmp(&other.id())
        }
    }
}

// -----------------------------------------------------------------------------
// ErasedAssetIndex

impl<A: Asset> TryFrom<&Handle<A>> for TypedAssetIndex {
    type Error = UuidNotSupportedError;

    fn try_from(handle: &Handle<A>) -> Result<Self, Self::Error> {
        match handle {
            Handle::Strong(handle) => Ok(Self {
                index: handle.index,
                type_id: handle.type_id,
            }),
            Handle::Uuid(uuid, ..) => Err(UuidNotSupportedError(*uuid)),
        }
    }
}

impl TryFrom<&ErasedHandle> for TypedAssetIndex {
    type Error = UuidNotSupportedError;

    fn try_from(handle: &ErasedHandle) -> Result<Self, Self::Error> {
        match handle {
            ErasedHandle::Strong(handle) => Ok(Self {
                index: handle.index,
                type_id: handle.type_id,
            }),
            ErasedHandle::Uuid { uuid, .. } => Err(UuidNotSupportedError(*uuid)),
        }
    }
}

// -----------------------------------------------------------------------------
// ErasedHandle & Handle - Conversion

/// Creates a [`Handle`] from a string literal containing a UUID.
///
/// # Examples
///
/// ```
/// # use voker_asset::{Handle, uuid_handle};
/// # type Image = ();
/// const IMAGE: Handle<Image> = uuid_handle!("1347c9b7-c46a-48e7-b7b8-023a354b7cac");
/// ```
#[macro_export]
macro_rules! uuid_handle {
    ($uuid:expr) => {{ $crate::Handle::Uuid($crate::uuid::uuid!($uuid), core::marker::PhantomData) }};
}
