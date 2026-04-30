use std::path::Path;

use alloc::vec::Vec;

use atomicow::CowArc;
use voker_utils::hash::{HashMap, HashSet};

use crate::asset::Asset;
use crate::handle::Handle;
use crate::ident::{ErasedAssetId, TypedAssetIndex};
use crate::io::Reader;
use crate::loader::{Deferred, LabeledAsset, LoadedAsset, NestedLoader, ReadAssetBytesError};
use crate::loader::{ErasedAssetLoader, ErasedLoadedAsset, ImmediateLoadError, StaticTyped};
use crate::meta::{AssetHash, DeserializeMetaError, ProcessedInfo, ProcessedInfoMinimal, Settings};
use crate::path::AssetPath;
use crate::server::{AssetServer, AssetServerMode};

pub struct LoadContext<'a> {
    // use `&AssetServerData` instead of `&AssetServer`
    // to reduce once indirect addressing
    pub(crate) asset_server: &'a AssetServer,
    pub(crate) populate_hashes: bool,
    pub(crate) should_load_dependencies: bool,
    pub(crate) asset_path: AssetPath<'static>,
    pub(crate) labeled_assets: Vec<LabeledAsset>,
    pub(crate) dependencies: HashSet<TypedAssetIndex>,
    pub(crate) loader_dependencies: HashMap<AssetPath<'static>, AssetHash>,
    /// Maps the label of a subasset to the index into [`Self::labeled_assets`].
    pub(crate) label_to_asset_index: HashMap<CowArc<'static, str>, usize>,
    /// Maps the subasset asset ID to the index into [`Self::labeled_assets`].
    pub(crate) asset_id_to_asset_index: HashMap<ErasedAssetId, usize>,
}

impl<'a> LoadContext<'a> {
    #[inline]
    pub(crate) fn new(
        asset_server: &'a AssetServer,
        asset_path: AssetPath<'static>,
        should_load_dependencies: bool,
        populate_hashes: bool,
    ) -> LoadContext<'a> {
        Self {
            asset_server,
            asset_path,
            populate_hashes,
            should_load_dependencies,
            labeled_assets: Vec::new(),
            dependencies: HashSet::new(),
            loader_dependencies: HashMap::new(),
            label_to_asset_index: HashMap::new(),
            asset_id_to_asset_index: HashMap::new(),
        }
    }

    #[inline]
    pub fn begin_labeled_asset(&self) -> LoadContext<'_> {
        Self {
            asset_server: self.asset_server,
            populate_hashes: self.populate_hashes,
            should_load_dependencies: self.should_load_dependencies,
            asset_path: self.asset_path.clone(),
            labeled_assets: Vec::new(),
            dependencies: HashSet::new(),
            loader_dependencies: HashMap::new(),
            label_to_asset_index: HashMap::new(),
            asset_id_to_asset_index: HashMap::new(),
        }
    }

    pub fn path(&self) -> &AssetPath<'static> {
        &self.asset_path
    }

    pub fn finish<A: Asset>(mut self, value: A) -> LoadedAsset<A> {
        value.visit_dependencies(&mut |asset_id| {
            match asset_id {
                ErasedAssetId::Index { type_id, index } => {
                    self.dependencies.insert(TypedAssetIndex { index, type_id });
                }
                ErasedAssetId::Uuid { .. } => {
                    // UUID assets can't be loaded anyway, so just ignore this ID.
                }
            };
        });

        LoadedAsset {
            value,
            dependencies: self.dependencies,
            loader_dependencies: self.loader_dependencies,
            labeled_assets: self.labeled_assets,
            label_to_asset_index: self.label_to_asset_index,
            asset_id_to_asset_index: self.asset_id_to_asset_index,
        }
    }

    pub fn labeled_asset_scope<A: Asset, E>(
        &mut self,
        label: impl Into<CowArc<'static, str>>,
        load: impl FnOnce(&mut LoadContext) -> Result<A, E>,
    ) -> Result<Handle<A>, E> {
        let mut context = self.begin_labeled_asset();
        let asset = load(&mut context)?;
        let loaded_asset = context.finish(asset);
        Ok(self.add_loaded_labeled_asset(label, loaded_asset))
    }

    pub fn add_labeled_asset<A: Asset>(
        &mut self,
        label: impl Into<CowArc<'static, str>>,
        asset: A,
    ) -> Handle<A> {
        // Manully inline, faster then `labeled_asset_scope`.
        let context = self.begin_labeled_asset();
        let loaded_asset = context.finish(asset);
        self.add_loaded_labeled_asset(label, loaded_asset)
    }

    pub fn add_loaded_labeled_asset<A: Asset>(
        &mut self,
        label: impl Into<CowArc<'static, str>>,
        loaded_asset: LoadedAsset<A>,
    ) -> Handle<A> {
        use voker_utils::hash::map::Entry;

        let label = label.into();
        let loaded_asset = loaded_asset.erased();
        let labeled_path = self.asset_path.clone().with_label(label.clone());

        let handle = self.asset_server.get_or_create_handle::<A>(labeled_path, None);

        let asset = LabeledAsset {
            asset: loaded_asset,
            handle: handle.clone().erased(),
        };

        match self.label_to_asset_index.entry(label) {
            Entry::Occupied(entry) => {
                // It seems unlikely someone wants to replace a subasset, we treat this as a bug.
                tracing::warn!(
                    "Duplicate label '{}' for asset '{}'. Replacing existing labeled asset. \
                    If it's unintended, it may indicate a bug in the asset or asset loader.",
                    entry.key(),
                    self.asset_path,
                );

                let index = *entry.get();
                // Note: we don't need to mess with the `asset_id_to_asset_index` here, since we
                // know the same path to `get_or_create_handle` will return the same handle as
                // long as the handle remains alive, and we hold the handle in `LabeledAsset`.
                self.labeled_assets[index] = asset;
            }
            Entry::Vacant(entry) => {
                entry.insert(self.labeled_assets.len());
                let key = handle.id().erased();
                let index = self.labeled_assets.len();
                self.asset_id_to_asset_index.insert(key, index);
                self.labeled_assets.push(asset);
            }
        }
        handle
    }

    pub fn has_labeled_asset<'b>(&self, label: impl Into<CowArc<'b, str>>) -> bool {
        let path = self.asset_path.clone().with_label(label.into());
        self.asset_server.contains_by_path(&path)
    }

    pub async fn read_asset_bytes<'b, 'c>(
        &'b mut self,
        path: impl Into<AssetPath<'c>>,
    ) -> Result<Vec<u8>, ReadAssetBytesError> {
        let path = path.into();
        if path.path() == Path::new("") {
            tracing::error!("Attempted to load an asset with an empty path \"{path}\"!");
            return Err(ReadAssetBytesError::EmptyPath(path.into_owned()));
        }

        let source = self.asset_server.get_source(path.source())?;
        let asset_reader = match self.asset_server.server_mode() {
            AssetServerMode::Unprocessed => source.reader(),
            AssetServerMode::Processed => source.processed_reader()?,
        };

        let mut reader = asset_reader.read(path.path()).await?;

        let hash: AssetHash = if self.populate_hashes {
            // NOTE: ensure meta is read while the asset bytes reader is still active to ensure transactionality
            // See `ProcessorGatedReader` for more info
            let meta_bytes = asset_reader.read_meta_bytes(path.path()).await?;
            let minimal: ProcessedInfoMinimal =
                ron::de::from_bytes(&meta_bytes).map_err(DeserializeMetaError::ProcessInfo)?;

            let processed_info = minimal.processed_info.ok_or_else(|| {
                core::hint::cold_path();
                ReadAssetBytesError::MissingAssetHash(path.clone_owned())
            })?;

            processed_info.full_hash
        } else {
            AssetHash::ZERO
        };

        let mut bytes = Vec::new();

        if let Err(err) = reader.read_all_bytes(&mut bytes).await {
            return Err(ReadAssetBytesError::Io {
                path: path.path().to_path_buf(),
                error: err,
            });
        }

        self.loader_dependencies.insert(path.clone_owned(), hash);
        Ok(bytes)
    }

    /// Returns a handle to an asset of type `A` with the label `label`.
    ///
    /// This [`LoadContext`] **must** produce an asset of the given type and the given label,
    /// otherwise the dependencies of this asset will never be considered "fully loaded".
    ///
    /// However you can call this method before _or_ after adding the labeled asset.
    pub fn get_label_handle<'b, A: Asset>(
        &mut self,
        label: impl Into<CowArc<'b, str>>,
    ) -> Handle<A> {
        let path = self.asset_path.clone().with_label(label);
        let handle = self.asset_server.get_or_create_handle::<A>(path, None);
        // `get_or_create_handle` always returns a Strong variant, so we are safe to unwrap.
        let index: TypedAssetIndex = (&handle).try_into().unwrap();
        self.dependencies.insert(index);
        handle
    }

    pub fn get_labeled(&self, label: impl AsRef<str>) -> Option<&ErasedLoadedAsset> {
        let index = self.label_to_asset_index.get(label.as_ref())?;
        let labeled = &self.labeled_assets[*index];
        Some(&labeled.asset)
    }

    pub fn get_labeled_by_id(&self, id: impl Into<ErasedAssetId>) -> Option<&ErasedLoadedAsset> {
        let index = self.asset_id_to_asset_index.get(&id.into())?;
        let labeled = &self.labeled_assets[*index];
        Some(&labeled.asset)
    }

    #[must_use]
    pub fn loader(&mut self) -> NestedLoader<'a, '_, StaticTyped, Deferred> {
        NestedLoader::new(self)
    }

    pub fn load<'b, A: Asset>(&mut self, path: impl Into<AssetPath<'b>>) -> Handle<A> {
        self.loader().load(path)
    }

    pub(crate) async fn load_direct_internal(
        &mut self,
        path: AssetPath<'static>,
        settings: &dyn Settings,
        loader: &dyn ErasedAssetLoader,
        reader: &mut dyn Reader,
        processed_info: Option<&ProcessedInfo>,
    ) -> Result<ErasedLoadedAsset, ImmediateLoadError> {
        let loaded_asset = self
            .asset_server
            .load_with_loader(
                &path,
                settings,
                loader,
                reader,
                self.should_load_dependencies,
                self.populate_hashes,
            )
            .await
            .map_err(|error| ImmediateLoadError::LoadError {
                dependency: path.clone(),
                error,
            })?;

        let hash = processed_info.map(|i| i.full_hash).unwrap_or_default();
        self.loader_dependencies.insert(path, hash);
        Ok(loaded_asset)
    }
}
