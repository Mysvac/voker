use alloc::vec::Vec;
use core::any::Any;
use core::borrow::Borrow;
use core::convert::Infallible;
use core::hash::Hash;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use atomicow::CowArc;
use serde::{Deserialize, Serialize};
use voker_ecs::error::GameError;
use voker_reflect::info::TypePath;
use voker_utils::hash::HashMap;

use crate::asset::Asset;
use crate::handle::{ErasedHandle, Handle};
use crate::ident::{AssetId, ErasedAssetId};
use crate::loader::{ErasedLoadedAsset, LabeledAsset};
use crate::meta::Settings;

// -----------------------------------------------------------------------------
// AssetTransformer

/// Transforms an [`Asset`] of [`AssetInput`] into [`AssetOutput`].
///
/// Commonly used with [`LoadTransformAndSave`] to build asset processing pipelines.
///
/// [`AssetInput`]: AssetTransformer::AssetInput
/// [`AssetOutput`]: AssetTransformer::AssetOutput
/// [`LoadTransformAndSave`]: crate::processor::LoadTransformAndSave
pub trait AssetTransformer: TypePath + Send + Sync + 'static {
    /// The input [`Asset`] type.
    type AssetInput: Asset;
    /// The output [`Asset`] type.
    type AssetOutput: Asset;
    /// Per-asset transformation settings.
    type Settings: Settings + Default + Serialize + for<'a> Deserialize<'a>;
    /// Error type returned on transformation failure.
    type Error: Into<GameError>;

    /// Transforms `asset` (and its labeled sub-assets) into [`Self::AssetOutput`].
    fn transform<'a>(
        &'a self,
        asset: TransformedAsset<Self::AssetInput>,
        settings: &'a Self::Settings,
    ) -> impl Future<Output = Result<TransformedAsset<Self::AssetOutput>, Self::Error>> + Send;
}

// -----------------------------------------------------------------------------
// TransformedAsset

/// An [`Asset`] (and its labeled sub-assets) being transformed in a processing pipeline.
pub struct TransformedAsset<A: Asset> {
    pub(crate) value: A,
    pub(crate) labeled_assets: Vec<LabeledAsset>,
    pub(crate) label_to_asset_index: HashMap<CowArc<'static, str>, usize>,
    pub(crate) asset_id_to_asset_index: HashMap<ErasedAssetId, usize>,
}

impl<A: Asset> Deref for TransformedAsset<A> {
    type Target = A;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<A: Asset> DerefMut for TransformedAsset<A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<A: Asset> TransformedAsset<A> {
    /// Creates a [`TransformedAsset`] from an [`ErasedLoadedAsset`].
    ///
    /// Returns `None` if the erased asset's type does not match `A`.
    pub fn from_loaded(asset: ErasedLoadedAsset) -> Option<Self> {
        let loaded = asset.downcast::<A>().ok()?;
        Some(TransformedAsset {
            value: loaded.value,
            labeled_assets: loaded.labeled_assets,
            label_to_asset_index: loaded.label_to_asset_index,
            asset_id_to_asset_index: loaded.asset_id_to_asset_index,
        })
    }

    /// Returns a reference to the asset value.
    #[inline]
    pub fn get(&self) -> &A {
        &self.value
    }

    /// Returns a mutable reference to the asset value.
    #[inline]
    pub fn get_mut(&mut self) -> &mut A {
        &mut self.value
    }

    /// Returns the typed [`Handle<B>`] of the labeled asset with the given `label`.
    pub fn get_handle<Q, B: Asset>(&self, label: &Q) -> Option<Handle<B>>
    where
        CowArc<'static, str>: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let index = *self.label_to_asset_index.get(label)?;
        Handle::<B>::try_from(self.labeled_assets[index].handle.clone()).ok()
    }

    /// Returns the [`ErasedHandle`] of the labeled asset with the given `label`.
    pub fn get_erased_handle<Q>(&self, label: &Q) -> Option<ErasedHandle>
    where
        CowArc<'static, str>: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let index = *self.label_to_asset_index.get(label)?;
        Some(self.labeled_assets[index].handle.clone())
    }

    /// Returns a mutable view of the labeled sub-asset with the given `label` downcast to `B`.
    pub fn get_labeled<B: Asset, Q>(&mut self, label: &Q) -> Option<TransformedSubAsset<'_, B>>
    where
        CowArc<'static, str>: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let index = *self.label_to_asset_index.get(label)?;
        let inner = &mut self.labeled_assets[index].asset;
        let value = <dyn Any>::downcast_mut::<B>(inner.value.as_mut())?;
        Some(TransformedSubAsset {
            value,
            labeled_assets: &mut inner.labeled_assets,
            label_to_asset_index: &mut inner.label_to_asset_index,
            asset_id_to_asset_index: &mut inner.asset_id_to_asset_index,
        })
    }

    /// Returns a type-erased reference to the labeled sub-asset with the given `label`.
    pub fn get_erased_labeled<Q>(&self, label: &Q) -> Option<&ErasedLoadedAsset>
    where
        CowArc<'static, str>: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let index = *self.label_to_asset_index.get(label)?;
        Some(&self.labeled_assets[index].asset)
    }

    /// Returns a mutable view of the labeled sub-asset for the given asset `id` downcast to `B`.
    pub fn get_labeled_by_id<B: Asset>(
        &mut self,
        id: impl Into<AssetId<B>>,
    ) -> Option<TransformedSubAsset<'_, B>> {
        let erased_id: ErasedAssetId = id.into().into();
        let index = *self.asset_id_to_asset_index.get(&erased_id)?;
        let inner = &mut self.labeled_assets[index].asset;
        let value = <dyn Any>::downcast_mut::<B>(&mut inner.value)?;
        Some(TransformedSubAsset {
            value,
            labeled_assets: &mut inner.labeled_assets,
            label_to_asset_index: &mut inner.label_to_asset_index,
            asset_id_to_asset_index: &mut inner.asset_id_to_asset_index,
        })
    }

    /// Returns a type-erased reference to the labeled sub-asset for the given asset `id`.
    pub fn get_erased_labeled_by_id(
        &self,
        id: impl Into<ErasedAssetId>,
    ) -> Option<&ErasedLoadedAsset> {
        let index = *self.asset_id_to_asset_index.get(&id.into())?;
        Some(&self.labeled_assets[index].asset)
    }

    /// Replaces the asset value with `asset`, transferring labeled sub-assets.
    pub fn replace_asset<B: Asset>(self, asset: B) -> TransformedAsset<B> {
        TransformedAsset {
            value: asset,
            labeled_assets: self.labeled_assets,
            label_to_asset_index: self.label_to_asset_index,
            asset_id_to_asset_index: self.asset_id_to_asset_index,
        }
    }

    /// Replaces this asset's labeled sub-assets with those from `source`.
    pub fn replace_labeled_assets<B: Asset>(&mut self, source: TransformedAsset<B>) {
        self.labeled_assets = source.labeled_assets;
        self.label_to_asset_index = source.label_to_asset_index;
        self.asset_id_to_asset_index = source.asset_id_to_asset_index;
    }

    /// Inserts or replaces a labeled sub-asset with the given `label`, `handle`, and `asset`.
    pub fn insert_labeled(
        &mut self,
        label: impl Into<CowArc<'static, str>>,
        handle: impl Into<ErasedHandle>,
        asset: impl Into<ErasedLoadedAsset>,
    ) {
        use voker_utils::hash::map::Entry;
        let labeled = LabeledAsset {
            asset: asset.into(),
            handle: handle.into(),
        };

        debug_assert_eq!(
            labeled.asset.asset_type_id(),
            labeled.handle.type_id(),
            "LabeledAsset type mismatched, handle is `{:?}`, asset is `{:?}`-`{}`",
            labeled.handle.type_id(),
            labeled.asset.asset_type_id(),
            labeled.asset.asset_type_path(),
        );

        match self.label_to_asset_index.entry(label.into()) {
            Entry::Occupied(entry) => {
                let index = *entry.get();
                let new_id = labeled.handle.id();
                let old_id = self.labeled_assets[index].handle.id();
                if new_id != old_id {
                    self.asset_id_to_asset_index.remove(&old_id);
                    self.asset_id_to_asset_index.insert(new_id, index);
                }
                self.labeled_assets[index] = labeled;
            }
            Entry::Vacant(entry) => {
                let index = self.labeled_assets.len();
                entry.insert(index);
                self.asset_id_to_asset_index.insert(labeled.handle.id(), index);
                self.labeled_assets.push(labeled);
            }
        }
    }

    /// Iterates over all labels of the labeled sub-assets.
    pub fn iter_labels(&self) -> impl ExactSizeIterator<Item = &str> {
        self.label_to_asset_index.keys().map(|s| &**s)
    }
}

// -----------------------------------------------------------------------------
// TransformedSubAsset

/// A mutable view into a labeled sub-asset of a [`TransformedAsset`].
pub struct TransformedSubAsset<'a, A: Asset> {
    value: &'a mut A,
    labeled_assets: &'a mut Vec<LabeledAsset>,
    label_to_asset_index: &'a mut HashMap<CowArc<'static, str>, usize>,
    asset_id_to_asset_index: &'a mut HashMap<ErasedAssetId, usize>,
}

impl<A: Asset> Deref for TransformedSubAsset<'_, A> {
    type Target = A;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<A: Asset> DerefMut for TransformedSubAsset<'_, A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

impl<'a, A: Asset> TransformedSubAsset<'a, A> {
    /// Creates a new [`TransformedSubAsset`] from `asset` if its internal value matches `A`.
    pub fn from_loaded(asset: &'a mut ErasedLoadedAsset) -> Option<Self> {
        let value = <dyn Any>::downcast_mut::<A>(asset.value.as_mut())?;
        Some(TransformedSubAsset {
            value,
            labeled_assets: &mut asset.labeled_assets,
            label_to_asset_index: &mut asset.label_to_asset_index,
            asset_id_to_asset_index: &mut asset.asset_id_to_asset_index,
        })
    }

    /// Returns a reference to the sub-asset value.
    #[inline]
    pub fn get(&self) -> &A {
        self.value
    }

    /// Returns a mutable reference to the sub-asset value.
    #[inline]
    pub fn get_mut(&mut self) -> &mut A {
        self.value
    }

    /// Returns the typed [`Handle<B>`] of the nested labeled asset with the given `label`.
    pub fn get_handle<Q, B: Asset>(&self, label: &Q) -> Option<Handle<B>>
    where
        CowArc<'static, str>: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let index = *self.label_to_asset_index.get(label)?;
        Handle::<B>::try_from(self.labeled_assets[index].handle.clone()).ok()
    }

    /// Returns the [`ErasedHandle`] of the nested labeled asset with the given `label`.
    pub fn get_erased_handle<Q>(&self, label: &Q) -> Option<ErasedHandle>
    where
        CowArc<'static, str>: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let index = *self.label_to_asset_index.get(label)?;
        Some(self.labeled_assets[index].handle.clone())
    }

    /// Returns a mutable view of the nested labeled sub-asset with the given `label` downcast to `B`.
    pub fn get_labeled<B: Asset, Q>(&mut self, label: &Q) -> Option<TransformedSubAsset<'_, B>>
    where
        CowArc<'static, str>: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let index = *self.label_to_asset_index.get(label)?;
        let inner = &mut self.labeled_assets[index].asset;
        let value = <dyn Any>::downcast_mut::<B>(inner.value.as_mut())?;
        Some(TransformedSubAsset {
            value,
            labeled_assets: &mut inner.labeled_assets,
            label_to_asset_index: &mut inner.label_to_asset_index,
            asset_id_to_asset_index: &mut inner.asset_id_to_asset_index,
        })
    }

    /// Returns a type-erased reference to the nested labeled sub-asset with the given `label`.
    pub fn get_erased_labeled<Q>(&self, label: &Q) -> Option<&ErasedLoadedAsset>
    where
        CowArc<'static, str>: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let index = *self.label_to_asset_index.get(label)?;
        Some(&self.labeled_assets[index].asset)
    }

    /// Returns a mutable view of the nested labeled sub-asset for the given asset `id` downcast to `B`.
    pub fn get_labeled_by_id<B: Asset>(
        &mut self,
        id: impl Into<AssetId<B>>,
    ) -> Option<TransformedSubAsset<'_, B>> {
        let erased_id: ErasedAssetId = id.into().into();
        let index = *self.asset_id_to_asset_index.get(&erased_id)?;
        let inner = &mut self.labeled_assets[index].asset;
        let value = <dyn Any>::downcast_mut::<B>(&mut inner.value)?;
        Some(TransformedSubAsset {
            value,
            labeled_assets: &mut inner.labeled_assets,
            label_to_asset_index: &mut inner.label_to_asset_index,
            asset_id_to_asset_index: &mut inner.asset_id_to_asset_index,
        })
    }

    /// Returns a type-erased reference to the nested labeled sub-asset for the given asset `id`.
    pub fn get_erased_labeled_by_id(
        &self,
        id: impl Into<ErasedAssetId>,
    ) -> Option<&ErasedLoadedAsset> {
        let index = *self.asset_id_to_asset_index.get(&id.into())?;
        Some(&self.labeled_assets[index].asset)
    }

    /// Inserts or replaces a nested labeled sub-asset.
    pub fn insert_labeled(
        &mut self,
        label: impl Into<CowArc<'static, str>>,
        handle: impl Into<ErasedHandle>,
        asset: impl Into<ErasedLoadedAsset>,
    ) {
        use voker_utils::hash::map::Entry;
        let labeled = LabeledAsset {
            asset: asset.into(),
            handle: handle.into(),
        };

        debug_assert_eq!(
            labeled.asset.asset_type_id(),
            labeled.handle.type_id(),
            "LabeledAsset type mismatched, handle is `{:?}`, asset is `{:?}`-`{}`",
            labeled.handle.type_id(),
            labeled.asset.asset_type_id(),
            labeled.asset.asset_type_path(),
        );

        match self.label_to_asset_index.entry(label.into()) {
            Entry::Occupied(entry) => {
                let index = *entry.get();
                let new_id = labeled.handle.id();
                let old_id = self.labeled_assets[index].handle.id();
                if new_id != old_id {
                    self.asset_id_to_asset_index.remove(&old_id);
                    self.asset_id_to_asset_index.insert(new_id, index);
                }
                self.labeled_assets[index] = labeled;
            }
            Entry::Vacant(entry) => {
                let index = self.labeled_assets.len();
                entry.insert(index);
                self.asset_id_to_asset_index.insert(labeled.handle.id(), index);
                self.labeled_assets.push(labeled);
            }
        }
    }

    /// Iterates over all labels of the nested labeled sub-assets.
    pub fn iter_labels(&self) -> impl ExactSizeIterator<Item = &str> {
        self.label_to_asset_index.keys().map(|s| &**s)
    }
}

// -----------------------------------------------------------------------------
// IdentityAssetTransformer

/// An [`AssetTransformer`] that returns the input asset unchanged.
///
/// Useful for format-conversion pipelines where no runtime transformation is needed.
#[derive(TypePath)]
#[type_path = "voker_asset::transformer::IdentityAssetTransformer"]
pub struct IdentityAssetTransformer<A: Asset> {
    _phantom: PhantomData<fn(A) -> A>,
}

impl<A: Asset> IdentityAssetTransformer<A> {
    /// Creates a new `IdentityAssetTransformer`.
    pub const fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<A: Asset> Default for IdentityAssetTransformer<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Asset> AssetTransformer for IdentityAssetTransformer<A> {
    type AssetInput = A;
    type AssetOutput = A;
    type Settings = ();
    type Error = Infallible;

    async fn transform(
        &self,
        asset: TransformedAsset<Self::AssetInput>,
        _settings: &Self::Settings,
    ) -> Result<TransformedAsset<Self::AssetOutput>, Self::Error> {
        Ok(asset)
    }
}
