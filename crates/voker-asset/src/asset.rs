use alloc::vec::Vec;

use voker_ecs::derive::SystemSet;
use voker_ecs::prelude::Component;
use voker_reflect::info::TypePath;
use voker_utils::hash::{HashMap, HashSet};

use crate::handle::{ErasedHandle, Handle};
use crate::ident::{AssetId, ErasedAssetId};

// -----------------------------------------------------------------------------
// Asset

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an `Asset`",
    label = "invalid `Asset`",
    note = "consider annotating `{Self}` with `#[derive(Asset)]`"
)]
pub trait Asset: VisitAssetDependencies + TypePath + Send + Sync + 'static {}

impl Asset for () {}

// -----------------------------------------------------------------------------
// VisitAssetDependencies

pub trait VisitAssetDependencies {
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
// VisitAssetDependencies

/// A trait for components that can be used as asset identifiers, e.g. handle wrappers.
pub trait AssetComponent: Component {
    /// The underlying asset type.
    type Asset: Asset;

    /// Retrieves the asset id from this component.
    fn asset_id(&self) -> AssetId<Self::Asset>;
}

// -----------------------------------------------------------------------------
// App Extension

/// System set for the server event handler that must run before [`AssetTrackingSystems`].
#[derive(SystemSet, Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetServerEventSystems;

#[derive(SystemSet, Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetTrackingSystems;

#[derive(SystemSet, Clone, Debug, Hash, PartialEq, Eq)]
pub struct AssetEventSystems;
