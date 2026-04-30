use alloc::vec::Vec;

use voker_ecs::derive::SystemSet;
use voker_ecs::prelude::Component;
use voker_reflect::info::TypePath;
use voker_utils::hash::{HashMap, HashSet};

use crate::handle::{ErasedHandle, Handle};
use crate::ident::{AssetId, ErasedAssetId};

// -----------------------------------------------------------------------------
// Asset

/// Marker trait for all data types that can be loaded and managed by [`AssetServer`].
///
/// Implementors must also implement [`VisitAssetDependencies`] and [`TypePath`] so
/// that the asset pipeline can discover and track sub-asset dependencies at runtime.
///
/// Derive with `#[derive(Asset)]` (from `voker-asset-derive`) alongside
/// `#[derive(Reflect)]` on the type:
///
/// ```rust
/// # use voker_asset::asset::Asset;
/// # use voker_reflect::prelude::*;
/// #[derive(Asset, Reflect)]
/// pub struct MyImage {
///     pub width:  u32,
///     pub height: u32,
///     pub pixels: Vec<u8>,
/// }
/// ```
///
/// [`AssetServer`]: crate::server::AssetServer
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an `Asset`",
    label = "invalid `Asset`",
    note = "consider annotating `{Self}` with `#[derive(Asset)]`"
)]
pub trait Asset: VisitAssetDependencies + TypePath + Send + Sync + 'static {}

impl Asset for () {}

// -----------------------------------------------------------------------------
// VisitAssetDependencies

/// Allows the asset pipeline to enumerate all [`AssetId`]s
/// that a value directly depends on.
///
/// The derive macro for [`Asset`] automatically implements this trait by inspecting
/// all `Handle<A>` and `ErasedHandle` fields.  Manual implementations are needed only
/// when dependencies are stored in a non-standard way (e.g. behind a custom smart
/// pointer).
pub trait VisitAssetDependencies {
    /// Call `visit` once for every asset this value depends on.
    fn visit_dependencies(&self, visit: &mut dyn FnMut(ErasedAssetId));
}

impl VisitAssetDependencies for () {
    fn visit_dependencies(&self, _: &mut dyn FnMut(ErasedAssetId)) {
        unreachable!()
    }
}

impl VisitAssetDependencies for ErasedAssetId {
    #[inline]
    fn visit_dependencies(&self, visit: &mut dyn FnMut(ErasedAssetId)) {
        visit(*self);
    }
}

impl VisitAssetDependencies for ErasedHandle {
    #[inline]
    fn visit_dependencies(&self, visit: &mut dyn FnMut(ErasedAssetId)) {
        visit(self.id());
    }
}

impl VisitAssetDependencies for Option<ErasedHandle> {
    #[inline]
    fn visit_dependencies(&self, visit: &mut dyn FnMut(ErasedAssetId)) {
        if let Some(handle) = self.as_ref() {
            visit(handle.id())
        }
    }
}

impl<A: Asset> VisitAssetDependencies for Handle<A> {
    #[inline]
    fn visit_dependencies(&self, visit: &mut dyn FnMut(ErasedAssetId)) {
        visit(self.id().erased());
    }
}

impl<A: Asset> VisitAssetDependencies for Option<Handle<A>> {
    #[inline]
    fn visit_dependencies(&self, visit: &mut dyn FnMut(ErasedAssetId)) {
        if let Some(handle) = self {
            visit(handle.id().erased());
        }
    }
}

impl<A: Asset, const N: usize> VisitAssetDependencies for [Handle<A>; N] {
    fn visit_dependencies(&self, visit: &mut dyn FnMut(ErasedAssetId)) {
        for dependency in self {
            visit(dependency.id().erased());
        }
    }
}

impl<const N: usize> VisitAssetDependencies for [ErasedHandle; N] {
    fn visit_dependencies(&self, visit: &mut dyn FnMut(ErasedAssetId)) {
        for dependency in self {
            visit(dependency.id());
        }
    }
}

impl<K, A: Asset> VisitAssetDependencies for HashMap<K, Handle<A>> {
    fn visit_dependencies(&self, visit: &mut dyn FnMut(ErasedAssetId)) {
        for dependency in self.values() {
            visit(dependency.id().erased());
        }
    }
}

impl<K> VisitAssetDependencies for HashMap<K, ErasedHandle> {
    fn visit_dependencies(&self, visit: &mut dyn FnMut(ErasedAssetId)) {
        for dependency in self.values() {
            visit(dependency.id());
        }
    }
}

impl<V: VisitAssetDependencies> VisitAssetDependencies for Vec<V> {
    fn visit_dependencies(&self, visit: &mut dyn FnMut(ErasedAssetId)) {
        for dependency in self {
            dependency.visit_dependencies(visit);
        }
    }
}

impl<V: VisitAssetDependencies> VisitAssetDependencies for HashSet<V> {
    fn visit_dependencies(&self, visit: &mut dyn FnMut(ErasedAssetId)) {
        for dependency in self {
            dependency.visit_dependencies(visit);
        }
    }
}

// -----------------------------------------------------------------------------
// AssetComponent

/// A trait for components that can be used as asset identifiers, e.g. handle wrappers.
pub trait AssetComponent: Component {
    /// The underlying asset type.
    type Asset: Asset;

    /// Retrieves the asset id from this component.
    fn asset_id(&self) -> AssetId<Self::Asset>;
}

// -----------------------------------------------------------------------------
// App Extension — System Sets

/// System set for the server event handler that must run before [`AssetTrackingSystems`].
///
/// Processes `AssetServerEvent`s (load completions, failures, hot-reload triggers)
/// received from background tasks and writes them into the ECS world.
#[derive(SystemSet, Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetServerEventSystems;

/// System set for tracking handle drops and updating `Assets<A>` storage.
///
/// Runs after [`AssetServerEventSystems`] so that new loads processed in the
/// same frame are visible before stale handles are recycled.
#[derive(SystemSet, Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetTrackingSystems;

/// System set for flushing queued [`AssetEvent`](crate::event::AssetEvent) messages.
///
/// Runs in `PostUpdate`, after all asset mutations for the frame have been applied.
#[derive(SystemSet, Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetEventSystems;
