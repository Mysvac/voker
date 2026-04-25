use alloc::vec::Vec;
use core::any::TypeId;

use alloc::sync::Arc;
use async_broadcast::RecvError;
use thiserror::Error;
use voker_reflect::info::TypePath;
use voker_task::IoTaskPool;
use voker_utils::extra::TypeIdMap;
use voker_utils::hash::HashMap;
use voker_utils::vec::SmallVec;

use crate::{
    loader::{AssetLoader, ErasedAssetLoader},
    path::AssetPath,
    utils::iter_secondary_extensions,
};

#[derive(Clone)]
pub(crate) enum MaybeAssetLoader {
    Ready(Arc<dyn ErasedAssetLoader>),
    Pending {
        sender: async_broadcast::Sender<Arc<dyn ErasedAssetLoader>>,
        receiver: async_broadcast::Receiver<Arc<dyn ErasedAssetLoader>>,
    },
}

impl MaybeAssetLoader {
    pub async fn get(self) -> Result<Arc<dyn ErasedAssetLoader>, GetLoaderError> {
        match self {
            Self::Ready(loader) => Ok(loader),
            Self::Pending { mut receiver, .. } => Ok(receiver.recv().await?),
        }
    }
}

#[derive(Error, Debug, Clone)]
pub(crate) enum GetLoaderError {
    #[error(transparent)]
    CouldNotResolve(#[from] RecvError),
}

#[derive(Default)]
pub(crate) struct AssetLoaders {
    loaders: Vec<MaybeAssetLoader>,
    extension_to_loaders: HashMap<&'static str, SmallVec<u32, 4>>,
    type_name_to_loader: HashMap<&'static str, Option<u32>>,
    type_path_to_loader: HashMap<&'static str, u32>,
    type_path_to_preregistered_loader: HashMap<&'static str, u32>,
    asset_type_id_to_loaders: TypeIdMap<Vec<u32>>,
}

impl AssetLoaders {
    /// Get the [`AssetLoader`] stored at the specific index
    fn get_by_index(&self, index: u32) -> Option<MaybeAssetLoader> {
        self.loaders.get(index as usize).cloned()
    }

    pub fn get_by_path(&self, path: &str) -> Option<MaybeAssetLoader> {
        let index = self.type_path_to_loader.get(path).copied()?;
        self.get_by_index(index)
    }

    pub fn get_by_name(&self, name: &str) -> Option<MaybeAssetLoader> {
        let index = self.type_name_to_loader.get(name).copied()??;
        self.get_by_index(index)
    }

    pub fn get_by_asset_type(&self, type_id: TypeId) -> Option<MaybeAssetLoader> {
        let index = self.asset_type_id_to_loaders.get(type_id)?.last().copied()?;
        self.get_by_index(index)
    }

    pub fn get_by_asset_path(&self, path: &AssetPath<'_>) -> Option<MaybeAssetLoader> {
        let full_extension = path.full_extension()?;

        core::iter::once(full_extension)
            .chain(iter_secondary_extensions(full_extension))
            .filter_map(|ext| self.extension_to_loaders.get(ext)?.last().copied())
            .find_map(|index| self.get_by_index(index))
    }

    pub fn get_by_extension(&self, extension: &str) -> Option<MaybeAssetLoader> {
        let index = self.extension_to_loaders.get(extension)?.last().copied()?;
        self.get_by_index(index)
    }

    fn map_extensions(
        &mut self,
        loader_index: u32,
        extensions: &[&'static str],
        asset_type_id: TypeId,
        asset_debugger: fn() -> &'static str,
    ) {
        let Some(loaders) = self.asset_type_id_to_loaders.get(asset_type_id) else {
            for &extension in extensions {
                self.extension_to_loaders
                    .entry(extension)
                    .or_default()
                    .push(loader_index);
            }
            return;
        };

        let mut duplicate_extensions = Vec::new();

        for &extension in extensions {
            let list = self.extension_to_loaders.entry(extension).or_default();

            if list.iter().any(|index| loaders.contains(index)) {
                duplicate_extensions.push(extension);
            }

            list.push(loader_index);
        }

        if !duplicate_extensions.is_empty() {
            let asset_type_path = asset_debugger();
            tracing::warn!(
                "Duplicate AssetLoader registered for Asset type `{asset_type_path}` \
                with extensions `{duplicate_extensions:?}`. Loader must be specified in a .meta \
                file in order to load assets of this type with these extensions."
            );
        }
    }

    fn map_names(
        &mut self,
        loader_index: u32,
        asset_type_id: TypeId,
        loader_type_path: &'static str,
        loader_type_name: &'static str,
    ) {
        use voker_utils::hash::map::Entry;

        self.type_path_to_preregistered_loader
            .insert(loader_type_path, loader_index);
        self.type_path_to_loader.insert(loader_type_path, loader_index);
        match self.type_name_to_loader.entry(loader_type_name) {
            Entry::Occupied(mut entry) => {
                entry.insert(None);
            }
            Entry::Vacant(entry) => {
                entry.insert(Some(loader_index));
            }
        };
        let entry = self.asset_type_id_to_loaders.entry(asset_type_id);
        entry.or_default().push(loader_index);
    }

    pub fn push<L: AssetLoader>(&mut self, loader: L) {
        let type_path = <L as TypePath>::type_path();
        let type_name = <L as TypePath>::type_name();
        let asset_type_id = TypeId::of::<L::Asset>();
        let loader = Arc::new(loader);

        // ------------------------------------------------------
        // Replace preregistered loader
        // ------------------------------------------------------

        if let Some(index) = self.type_path_to_preregistered_loader.remove(type_path) {
            let replaced = core::mem::replace(
                self.loaders.get_mut(index as usize).unwrap(),
                MaybeAssetLoader::Ready(loader.clone()),
            );
            match replaced {
                MaybeAssetLoader::Ready(_) => {
                    unreachable!("duplicated push is not allowed")
                }
                MaybeAssetLoader::Pending { sender, .. } => {
                    IoTaskPool::get()
                        .spawn(async move {
                            let _ = sender.broadcast(loader).await;
                        })
                        .detach();
                }
            }
            // Try to dealloc memory.
            if self.type_path_to_preregistered_loader.is_empty() {
                self.type_path_to_preregistered_loader = HashMap::new();
            }
            return;
        }

        // ------------------------------------------------------
        // Check duplicates
        // ------------------------------------------------------

        let Ok(loader_index) = u32::try_from(self.loaders.len()) else {
            core::hint::cold_path();
            unreachable!("too many asset loaders");
        };

        let extensions = AssetLoader::extensions(&*loader);
        let asset_debugger = <L::Asset as TypePath>::type_path;
        self.map_extensions(loader_index, extensions, asset_type_id, asset_debugger);

        // ------------------------------------------------------
        // Add Loader object
        // ------------------------------------------------------

        self.loaders.push(MaybeAssetLoader::Ready(loader));
        self.map_names(loader_index, asset_type_id, type_path, type_name);
    }

    pub fn reserve<L: AssetLoader>(&mut self, extensions: &[&'static str]) {
        let type_path = <L as TypePath>::type_path();
        let type_name = <L as TypePath>::type_name();
        let asset_type_id = TypeId::of::<L::Asset>();

        // ------------------------------------------------------
        // Check duplicates
        // ------------------------------------------------------

        let Ok(loader_index) = u32::try_from(self.loaders.len()) else {
            core::hint::cold_path();
            unreachable!("too many asset loaders");
        };

        let asset_debugger = <L::Asset as TypePath>::type_path;
        self.map_extensions(loader_index, extensions, asset_type_id, asset_debugger);

        // ------------------------------------------------------
        // Add Loader object
        // ------------------------------------------------------

        let (mut sender, receiver) = async_broadcast::broadcast(1);
        sender.set_overflow(true);

        self.loaders.push(MaybeAssetLoader::Pending { sender, receiver });
        self.map_names(loader_index, asset_type_id, type_path, type_name);
    }

    pub fn find(
        &self,
        type_path: Option<&str>,
        type_name: Option<&str>,
        asset_type_id: Option<TypeId>,
        extension: Option<&str>,
        asset_path: Option<&AssetPath<'_>>,
    ) -> Option<MaybeAssetLoader> {
        // ------------------------------------------------------
        // TypePath or TypeName
        // ------------------------------------------------------

        if let Some(type_path) = type_path {
            return self.get_by_path(type_path);
        }

        if let Some(type_name) = type_name
            && let Some(loader) = self.get_by_name(type_name)
        {
            return Some(loader);
        }

        // ------------------------------------------------------
        // TypeId (If assets do not have labels)
        // ------------------------------------------------------

        let label = asset_path.as_ref().and_then(|path| path.label());

        let mut candidates = None;

        if label.is_none()
            && let Some(type_id) = asset_type_id
        {
            candidates = Some(self.asset_type_id_to_loaders.get(type_id)?);
        };

        if let Some(candidates) = candidates {
            if candidates.is_empty() {
                return None;
            } else if candidates.len() == 1 {
                let index = candidates.first().copied().unwrap();
                return self.get_by_index(index);
            }
        }

        // ------------------------------------------------------
        // Extension
        // ------------------------------------------------------

        let try_extension = |extension: &str| {
            if let Some(indices) = self.extension_to_loaders.get(extension) {
                if let Some(candidates) = candidates
                    && !candidates.is_empty()
                {
                    indices.iter().rev().find(|index| candidates.contains(index))
                } else {
                    indices.last()
                }
            } else {
                None
            }
        };

        // explicit extension
        if let Some(extension) = extension
            && let Some(&index) = try_extension(extension)
        {
            return self.get_by_index(index);
        }

        // asset extensions
        if let Some(full_extension) = asset_path.and_then(AssetPath::full_extension) {
            if let Some(&index) = try_extension(full_extension) {
                return self.get_by_index(index);
            }

            // Try secondary extensions from the path
            for extension in iter_secondary_extensions(full_extension) {
                if let Some(&index) = try_extension(extension) {
                    return self.get_by_index(index);
                }
            }
        }

        // ------------------------------------------------------
        // Fallback, from TypeId if exists
        // ------------------------------------------------------

        let fallback = candidates?.last().copied().and_then(|index| self.get_by_index(index));

        match fallback {
            Some(loader) => {
                tracing::warn!(
                    "Multiple AssetLoaders found for Asset: {:?}; Path: {:?}; Extension: {:?}",
                    asset_type_id,
                    asset_path,
                    extension
                );
                Some(loader)
            }
            None => {
                tracing::warn!(
                    "No AssetLoader found for Asset: {:?}; Path: {:?}; Extension: {:?}",
                    asset_type_id,
                    asset_path,
                    extension
                );
                None
            }
        }
    }
}
