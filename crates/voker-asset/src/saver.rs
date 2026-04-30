use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::{Any, TypeId};
use core::ops::Deref;

use alloc::sync::Arc;
use atomicow::CowArc;
use futures_lite::AsyncWriteExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use voker_ecs::error::GameError;
use voker_reflect::info::TypePath;
use voker_utils::hash::HashMap;

use crate::BoxedFuture;
use crate::asset::Asset;
use crate::handle::{ErasedHandle, Handle};
use crate::ident::{AssetId, ErasedAssetId};
use crate::io::{AssetWriterError, MissingAssetSource, MissingAssetWriter, Writer};
use crate::loader::AssetLoader;
use crate::loader::{AssetContainer, ErasedLoadedAsset, LabeledAsset};
use crate::meta::{AssetConfig, AssetMeta, Settings};
use crate::path::AssetPath;
use crate::server::AssetServer;
use crate::transformer::TransformedAsset;

// -----------------------------------------------------------------------------
// AssetSaver

/// Writes a runtime [`Asset`] to bytes that can later be reloaded with [`Loader`].
///
/// Used in asset processing pipelines, typically via [`LoadTransformAndSave`].
///
/// [`Loader`]: AssetSaver::Loader
/// [`LoadTransformAndSave`]: crate::processor::LoadTransformAndSave
pub trait AssetSaver: TypePath + Send + Sync + 'static {
    /// The [`Asset`] type this saver writes.
    type Asset: Asset;
    /// Per-asset saver settings.
    type Settings: Settings + Default + Serialize + for<'a> Deserialize<'a>;
    /// The [`AssetLoader`] that will load the output bytes.
    type Loader: AssetLoader;
    /// Error type returned on save failure.
    type Error: Into<GameError>;

    /// Writes `asset` to `writer` using `settings` and returns the loader settings for the output.
    fn save(
        &self,
        asset: SavedAsset<'_, '_, Self::Asset>,
        writer: &mut dyn Writer,
        settings: &Self::Settings,
        asset_path: AssetPath<'_>,
    ) -> impl Future<Output = Result<<Self::Loader as AssetLoader>::Settings, Self::Error>> + Send;
}

// -----------------------------------------------------------------------------
// ErasedAssetSaver

/// A type-erased variant of [`AssetSaver`].
pub trait ErasedAssetSaver: Send + Sync + 'static {
    /// Type-erased variant of [`AssetSaver::save`].
    fn save<'a>(
        &'a self,
        asset: &'a ErasedLoadedAsset,
        writer: &'a mut dyn Writer,
        settings: &'a dyn Settings,
        asset_path: AssetPath<'a>,
    ) -> BoxedFuture<'a, Result<(), GameError>>;

    /// The type path of this saver.
    fn type_path(&self) -> &'static str;
}

impl<S: AssetSaver> ErasedAssetSaver for S {
    fn save<'a>(
        &'a self,
        asset: &'a ErasedLoadedAsset,
        writer: &'a mut dyn Writer,
        settings: &'a dyn Settings,
        asset_path: AssetPath<'a>,
    ) -> BoxedFuture<'a, Result<(), GameError>> {
        Box::pin(async move {
            let settings = <dyn Any>::downcast_ref::<S::Settings>(settings)
                .expect("AssetSaver settings should match the saver type");

            let saved_asset = SavedAsset::<S::Asset>::from_loaded(asset)
                .expect("AssetSaver asset type should match");

            self.save(saved_asset, writer, settings, asset_path)
                .await
                .map(|_| ())
                .map_err(Into::into)
        })
    }

    fn type_path(&self) -> &'static str {
        <S as TypePath>::type_path()
    }
}

// -----------------------------------------------------------------------------
// Moo (maybe-owned-object)

/// A value that is either owned or borrowed.
///
/// Unlike [`Cow`](alloc::borrow::Cow), this works with any type and avoids
/// invariance issues caused by [`ToOwned`](alloc::borrow::ToOwned) associated types.
#[derive(Clone)]
enum Moo<'a, T> {
    Owned(T),
    Borrowed(&'a T),
}

impl<T> Deref for Moo<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        match self {
            Moo::Owned(t) => t,
            Moo::Borrowed(t) => t,
        }
    }
}

// -----------------------------------------------------------------------------
// LabeledSavedAsset (private)

#[derive(Clone)]
struct LabeledSavedAsset<'a> {
    asset: ErasedSavedAsset<'a, 'a>,
    handle: ErasedHandle,
}

impl<'a> LabeledSavedAsset<'a> {
    fn from_labeled_asset(asset: &'a LabeledAsset) -> Self {
        Self {
            handle: asset.handle.clone(),
            asset: ErasedSavedAsset::from_loaded(&asset.asset),
        }
    }
}

// -----------------------------------------------------------------------------
// SavedAsset

/// An [`Asset`] (and its labeled sub-assets) ready to be saved.
#[derive(Clone)]
pub struct SavedAsset<'a, 'b, A: Asset> {
    value: &'a A,
    labeled_assets: Moo<'b, Vec<LabeledSavedAsset<'a>>>,
    label_to_asset_index: Moo<'b, HashMap<CowArc<'a, str>, usize>>,
    asset_id_to_asset_index: Moo<'b, HashMap<ErasedAssetId, usize>>,
}

impl<A: Asset> Deref for SavedAsset<'_, '_, A> {
    type Target = A;
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, 'b, A: Asset> SavedAsset<'a, 'b, A> {
    fn from_value_and_labeled_saved_assets(
        value: &'a A,
        labeled_assets: &'b Vec<LabeledSavedAsset<'a>>,
        label_to_asset_index: &'b HashMap<CowArc<'a, str>, usize>,
        asset_id_to_asset_index: &'b HashMap<ErasedAssetId, usize>,
    ) -> Self {
        Self {
            value,
            labeled_assets: Moo::Borrowed(labeled_assets),
            label_to_asset_index: Moo::Borrowed(label_to_asset_index),
            asset_id_to_asset_index: Moo::Borrowed(asset_id_to_asset_index),
        }
    }

    fn from_value_and_labeled_assets(
        value: &'a A,
        labeled_assets: &'a [LabeledAsset],
        label_to_asset_index: &'a HashMap<CowArc<'static, str>, usize>,
        asset_id_to_asset_index: &'a HashMap<ErasedAssetId, usize>,
    ) -> Self {
        Self {
            value,
            labeled_assets: Moo::Owned(
                labeled_assets
                    .iter()
                    .map(LabeledSavedAsset::from_labeled_asset)
                    .collect(),
            ),
            label_to_asset_index: Moo::Borrowed(label_to_asset_index),
            asset_id_to_asset_index: Moo::Borrowed(asset_id_to_asset_index),
        }
    }

    /// Creates a [`SavedAsset`] from an [`ErasedLoadedAsset`].
    ///
    /// Returns `None` if the asset's type does not match `A`.
    pub fn from_loaded(asset: &'a ErasedLoadedAsset) -> Option<Self> {
        let value = asset.get::<A>()?;
        Some(Self::from_value_and_labeled_assets(
            value,
            &asset.labeled_assets,
            &asset.label_to_asset_index,
            &asset.asset_id_to_asset_index,
        ))
    }

    /// Creates a [`SavedAsset`] from a [`TransformedAsset`].
    pub fn from_transformed(asset: &'a TransformedAsset<A>) -> Self {
        Self::from_value_and_labeled_assets(
            &asset.value,
            &asset.labeled_assets,
            &asset.label_to_asset_index,
            &asset.asset_id_to_asset_index,
        )
    }

    /// Creates a [`SavedAsset`] holding only `value` with no labeled sub-assets.
    pub fn from_asset(value: &'a A) -> Self {
        Self {
            value,
            labeled_assets: Moo::Owned(Vec::new()),
            label_to_asset_index: Moo::Owned(HashMap::new()),
            asset_id_to_asset_index: Moo::Owned(HashMap::new()),
        }
    }

    /// Converts this typed asset into a type-erased [`ErasedSavedAsset`].
    pub fn erased(self) -> ErasedSavedAsset<'a, 'a>
    where
        'b: 'a,
    {
        ErasedSavedAsset {
            value: self.value,
            labeled_assets: self.labeled_assets,
            label_to_asset_index: self.label_to_asset_index,
            asset_id_to_asset_index: self.asset_id_to_asset_index,
        }
    }

    /// Returns a reference to the asset value.
    #[inline]
    pub fn get(&self) -> &'a A {
        self.value
    }

    /// Returns the labeled sub-asset with the given `label` downcast to `B`.
    pub fn get_labeled<B: Asset>(&self, label: impl AsRef<str>) -> Option<SavedAsset<'a, '_, B>> {
        let index = self.label_to_asset_index.get(label.as_ref())?;
        let labeled = &self.labeled_assets[*index];
        labeled.asset.downcast()
    }

    /// Returns the type-erased labeled sub-asset with the given `label`.
    pub fn get_erased_labeled(&self, label: impl AsRef<str>) -> Option<&ErasedSavedAsset<'a, '_>> {
        let index = self.label_to_asset_index.get(label.as_ref())?;
        Some(&self.labeled_assets[*index].asset)
    }

    /// Returns the labeled sub-asset for the given asset `id` downcast to `B`.
    pub fn get_labeled_by_id<B: Asset>(
        &self,
        id: impl Into<AssetId<B>>,
    ) -> Option<SavedAsset<'a, '_, B>> {
        let erased_id: ErasedAssetId = id.into().into();
        let index = self.asset_id_to_asset_index.get(&erased_id)?;
        let labeled = &self.labeled_assets[*index];
        labeled.asset.downcast()
    }

    /// Returns the type-erased labeled sub-asset for the given asset `id`.
    pub fn get_erased_labeled_by_id(
        &self,
        id: impl Into<ErasedAssetId>,
    ) -> Option<&ErasedSavedAsset<'a, '_>> {
        let index = self.asset_id_to_asset_index.get(&id.into())?;
        Some(&self.labeled_assets[*index].asset)
    }

    /// Returns the typed [`Handle<B>`] of the labeled asset with the given `label`.
    pub fn get_handle<B: Asset>(&self, label: impl AsRef<str>) -> Option<Handle<B>> {
        let index = self.label_to_asset_index.get(label.as_ref())?;
        Handle::<B>::try_from(self.labeled_assets[*index].handle.clone()).ok()
    }

    /// Returns the [`ErasedHandle`] of the labeled asset with the given `label`.
    pub fn get_erased_handle(&self, label: impl AsRef<str>) -> Option<ErasedHandle> {
        let index = self.label_to_asset_index.get(label.as_ref())?;
        Some(self.labeled_assets[*index].handle.clone())
    }

    /// Iterates over all labels of the labeled sub-assets.
    pub fn iter_labels(&self) -> impl ExactSizeIterator<Item = &str> {
        self.label_to_asset_index.keys().map(|s| &**s)
    }
}

// -----------------------------------------------------------------------------
// ErasedSavedAsset

/// A type-erased [`SavedAsset`].
#[derive(Clone)]
pub struct ErasedSavedAsset<'a, 'b> {
    value: &'a dyn AssetContainer,
    labeled_assets: Moo<'b, Vec<LabeledSavedAsset<'a>>>,
    label_to_asset_index: Moo<'b, HashMap<CowArc<'a, str>, usize>>,
    asset_id_to_asset_index: Moo<'b, HashMap<ErasedAssetId, usize>>,
}

impl<'a, 'b> ErasedSavedAsset<'a, 'b> {
    /// Returns a reference to the asset value.
    ///
    /// Return `None` is type mismatched.
    #[inline]
    pub fn get<A: Asset>(&self) -> Option<&'a A> {
        <dyn Any>::downcast_ref(self.value)
    }

    /// Returns the labeled sub-asset with the given `label` downcast to `B`.
    pub fn get_labeled<B: Asset>(&self, label: impl AsRef<str>) -> Option<SavedAsset<'a, '_, B>> {
        let index = self.label_to_asset_index.get(label.as_ref())?;
        let labeled = &self.labeled_assets[*index];
        labeled.asset.downcast()
    }

    /// Returns the type-erased labeled sub-asset with the given `label`.
    pub fn get_erased_labeled(&self, label: impl AsRef<str>) -> Option<&ErasedSavedAsset<'a, '_>> {
        let index = self.label_to_asset_index.get(label.as_ref())?;
        Some(&self.labeled_assets[*index].asset)
    }

    /// Returns the labeled sub-asset for the given asset `id` downcast to `B`.
    pub fn get_labeled_by_id<B: Asset>(
        &self,
        id: impl Into<AssetId<B>>,
    ) -> Option<SavedAsset<'a, '_, B>> {
        let erased_id: ErasedAssetId = id.into().into();
        let index = self.asset_id_to_asset_index.get(&erased_id)?;
        let labeled = &self.labeled_assets[*index];
        labeled.asset.downcast()
    }

    /// Returns the type-erased labeled sub-asset for the given asset `id`.
    pub fn get_erased_labeled_by_id(
        &self,
        id: impl Into<ErasedAssetId>,
    ) -> Option<&ErasedSavedAsset<'a, '_>> {
        let index = self.asset_id_to_asset_index.get(&id.into())?;
        Some(&self.labeled_assets[*index].asset)
    }

    /// Returns the typed [`Handle<B>`] of the labeled asset with the given `label`.
    pub fn get_handle<B: Asset>(&self, label: impl AsRef<str>) -> Option<Handle<B>> {
        let index = self.label_to_asset_index.get(label.as_ref())?;
        Handle::<B>::try_from(self.labeled_assets[*index].handle.clone()).ok()
    }

    /// Returns the [`ErasedHandle`] of the labeled asset with the given `label`.
    pub fn get_erased_handle(&self, label: impl AsRef<str>) -> Option<ErasedHandle> {
        let index = self.label_to_asset_index.get(label.as_ref())?;
        Some(self.labeled_assets[*index].handle.clone())
    }

    /// Iterates over all labels of the labeled sub-assets.
    pub fn iter_labels(&self) -> impl ExactSizeIterator<Item = &str> {
        self.label_to_asset_index.keys().map(|s| &**s)
    }
}

impl<'a> ErasedSavedAsset<'a, '_> {
    /// Creates a [`ErasedSavedAsset`] from an [`ErasedLoadedAsset`].
    pub fn from_loaded(asset: &'a ErasedLoadedAsset) -> Self {
        Self {
            value: &*asset.value,
            labeled_assets: Moo::Owned(
                asset
                    .labeled_assets
                    .iter()
                    .map(LabeledSavedAsset::from_labeled_asset)
                    .collect(),
            ),
            label_to_asset_index: Moo::Borrowed(&asset.label_to_asset_index),
            asset_id_to_asset_index: Moo::Borrowed(&asset.asset_id_to_asset_index),
        }
    }

    /// Attempts to downcast this erased asset into typed [`SavedAsset<A>`].
    ///
    /// Returns `None` if the asset is not of type `A`.
    pub fn downcast<'b, A: Asset>(&'b self) -> Option<SavedAsset<'a, 'b, A>> {
        let value = <dyn Any>::downcast_ref::<A>(self.value)?;
        Some(SavedAsset::from_value_and_labeled_saved_assets(
            value,
            &self.labeled_assets,
            &self.label_to_asset_index,
            &self.asset_id_to_asset_index,
        ))
    }
}

impl<'a, 'b, A: Asset> From<SavedAsset<'a, 'b, A>> for ErasedSavedAsset<'a, 'b> {
    fn from(value: SavedAsset<'a, 'b, A>) -> Self {
        Self {
            value: value.value,
            labeled_assets: value.labeled_assets,
            label_to_asset_index: value.label_to_asset_index,
            asset_id_to_asset_index: value.asset_id_to_asset_index,
        }
    }
}

// -----------------------------------------------------------------------------
// SavedAssetBuilder

/// Builder for constructing [`SavedAsset`] instances when saving assets.
pub struct SavedAssetBuilder<'a> {
    asset_server: AssetServer,
    asset_path: AssetPath<'static>,
    labeled_assets: Vec<LabeledSavedAsset<'a>>,
    label_to_asset_index: HashMap<CowArc<'a, str>, usize>,
    asset_id_to_asset_index: HashMap<ErasedAssetId, usize>,
}

impl<'a> SavedAssetBuilder<'a> {
    /// Creates a new empty builder.
    pub fn new(asset_server: AssetServer, mut asset_path: AssetPath<'static>) -> Self {
        asset_path.remove_label();
        Self {
            asset_server,
            asset_path,
            labeled_assets: Vec::new(),
            label_to_asset_index: HashMap::new(),
            asset_id_to_asset_index: HashMap::new(),
        }
    }

    pub fn add_labeled_new_handle<'b: 'a, A: Asset>(
        &mut self,
        label: impl Into<CowArc<'b, str>>,
        asset: SavedAsset<'a, 'a, A>,
    ) -> Handle<A> {
        let label = label.into();
        let type_id = TypeId::of::<A>();
        let path = self.asset_path.clone().with_label(label.clone()).into_owned();
        let infos = self.asset_server.read_infos();
        let provider = infos
            .handle_providers
            .get(type_id)
            .expect("asset type has been initialized");
        let strong = provider.alloc_handle(false, Some(path), None);
        ::core::mem::drop(infos);
        let handle = Handle::<A>::Strong(strong);
        let erased_handle = handle.clone().erased();
        let erased_asset = asset.erased();
        self.add_labeled(label, erased_asset, erased_handle);
        handle
    }

    pub fn add_labeled_new_erased_handle<'b: 'a>(
        &mut self,
        label: impl Into<CowArc<'b, str>>,
        asset: ErasedSavedAsset<'a, 'a>,
    ) -> ErasedHandle {
        let label = label.into();
        let type_id = asset.value.type_id();
        let path = self.asset_path.clone().with_label(label.clone()).into_owned();
        let infos = self.asset_server.read_infos();
        let provider = infos
            .handle_providers
            .get(type_id)
            .expect("asset type has been initialized");
        let strong = provider.alloc_handle(false, Some(path), None);
        ::core::mem::drop(infos);
        let handle = ErasedHandle::Strong(strong);
        self.add_labeled(label, asset, handle.clone());
        handle
    }

    pub fn add_labeled<'b: 'a>(
        &mut self,
        label: impl Into<CowArc<'b, str>>,
        asset: impl Into<ErasedSavedAsset<'a, 'a>>,
        handle: impl Into<ErasedHandle>,
    ) {
        use voker_utils::hash::map::Entry;

        let labeled = LabeledSavedAsset {
            asset: asset.into(),
            handle: handle.into(),
        };

        debug_assert_eq!(
            labeled.asset.value.type_id(),
            labeled.handle.type_id(),
            "LabeledAsset type mismatched, handle is `{:?}`, asset is `{:?}`",
            labeled.asset.value.type_id(),
            labeled.handle.type_id(),
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

    /// Builds the final [`SavedAsset`] wrapping `asset`.
    pub fn build<'b, A: Asset>(self, asset: &'b A) -> SavedAsset<'b, 'b, A>
    where
        'a: 'b,
    {
        SavedAsset {
            value: asset,
            labeled_assets: Moo::Owned(self.labeled_assets),
            label_to_asset_index: Moo::Owned(self.label_to_asset_index),
            asset_id_to_asset_index: Moo::Owned(self.asset_id_to_asset_index),
        }
    }
}

// -----------------------------------------------------------------------------
// SaveAssetError

/// Error encountered while saving an asset via [`save_using_saver`].
#[derive(Error, Debug)]
pub enum SaveAssetError {
    #[error(transparent)]
    MissingSource(#[from] MissingAssetSource),
    #[error(transparent)]
    MissingWriter(#[from] MissingAssetWriter),
    #[error(transparent)]
    WriterError(#[from] AssetWriterError),
    #[error("Failed to save asset: {0}")]
    SaverError(Arc<GameError>),
}

// -----------------------------------------------------------------------------
// save_using_saver

/// Saves `asset` to `path` using the provided `saver` and `settings`.
pub async fn save_using_saver<S: AssetSaver>(
    asset_server: AssetServer,
    saver: &S,
    path: &AssetPath<'_>,
    asset: SavedAsset<'_, '_, S::Asset>,
    settings: &S::Settings,
) -> Result<(), SaveAssetError> {
    let source = asset_server.get_source(path.source())?;
    let asset_writer = source.writer()?;
    let mut writer = asset_writer.write(path.path()).await?;

    let settings = saver
        .save(asset, &mut writer, settings, path.clone())
        .await
        .map_err(|err| SaveAssetError::SaverError(Arc::new(err.into())))?;

    writer.flush().await.map_err(AssetWriterError::Io)?;

    let loader = S::Loader::type_path().into();
    let config = AssetConfig::Load { loader, settings };
    let meta = AssetMeta::<S::Loader, ()>::new(config);

    let meta_bytes = meta.serialize();
    asset_writer.write_meta_bytes(path.path(), &meta_bytes).await?;

    Ok(())
}
