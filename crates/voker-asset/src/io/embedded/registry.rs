use alloc::boxed::Box;
use std::path::{Path, PathBuf};

use voker_ecs::derive::Resource;

use crate::io::memory::{Data, Dir, MemoryAssetReader, Value};
use crate::io::{AssetSourceBuilder, AssetSourceBuilders};
use crate::io::{EMBEDDED, ErasedAssetReader};

cfg_select! {
    all(feature = "file_watcher", not(target_arch = "wasm32"), not(target_os = "android")) => {
        use alloc::sync::Arc;
        use voker_os::sync::{PoisonError, RwLock};
        use voker_utils::hash::HashMap;
    }
    _ => {}
}

// -----------------------------------------------------------------------------
// EmbeddedAssetRegistry

/// A [`Resource`] that manages embedded assets in a virtual in memory [`Dir`].
///
/// Generally this should not be interacted with directly. The [`embedded_asset`]
/// and [`load_embedded_asset`] macros will populate this.
///
/// [`Resource`]: trait@voker_ecs::resource::Resource
/// [`embedded_asset`]: crate::embedded_asset
/// [`load_embedded_asset`]: crate::load_embedded_asset
#[derive(Resource, Default)]
pub struct EmbeddedAssetRegistry {
    dir: Dir,

    #[cfg(all(
        feature = "file_watcher",
        not(target_arch = "wasm32"),
        not(target_os = "android")
    ))]
    root_paths: Arc<RwLock<HashMap<Box<Path>, PathBuf>>>,
}

impl EmbeddedAssetRegistry {
    /// Inserts new asset with `full_path`, `asset_path` and `value`.
    ///
    /// The full path as [`file!`] would return for that file, if it was capable of
    /// running in a non-rust file. `asset_path` is the path that will be used to
    /// identify the asset in the `embedded` [`AssetSource`]. `value` is the bytes
    /// that will be returned for the asset. This can be _either_ a `&'static [u8]`
    /// , a `Vec<u8>` or a `Arc<[u8]>`.
    ///
    /// [`AssetSource`]: crate::io::AssetSource
    pub fn insert_asset(&self, full_path: PathBuf, asset_path: &Path, value: impl Into<Value>) {
        #[cfg(any(
            not(feature = "file_watcher"),
            target_arch = "wasm32",
            target_os = "android"
        ))]
        let _ = full_path;

        #[cfg(all(
            feature = "file_watcher",
            not(target_arch = "wasm32"),
            not(target_os = "android")
        ))]
        self.root_paths
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(full_path.into(), asset_path.to_path_buf());

        self.dir.insert_asset(asset_path, value);
    }

    /// Inserts new asset metadata with `full_path`, `asset_path` and `value`.
    ///
    /// The full path as [`file!`] would return for that file, if it was capable of
    /// running in a non-rust file. `asset_path` is the path that will be used to
    /// identify the asset in the `embedded` [`AssetSource`]. `value` is the bytes
    /// that will be returned for the asset. This can be _either_ a `&'static [u8]`
    /// , a `Vec<u8>` or a `Arc<[u8]>`.
    ///
    /// [`AssetSource`]: crate::io::AssetSource
    pub fn insert_meta(&self, full_path: &Path, asset_path: &Path, value: impl Into<Value>) {
        #[cfg(any(
            not(feature = "file_watcher"),
            target_arch = "wasm32",
            target_os = "android"
        ))]
        let _ = full_path;

        #[cfg(all(
            feature = "file_watcher",
            not(target_arch = "wasm32"),
            not(target_os = "android")
        ))]
        self.root_paths
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(full_path.into(), asset_path.to_path_buf());

        self.dir.insert_meta(asset_path, value);
    }

    /// Removes an asset stored using `full_path`.
    ///
    /// The full path as [`file!`] would return for that file, if it was capable of
    /// running in a non-rust file. If no asset is stored with at `full_path` its a
    /// no-op. It returning `Option` contains the originally stored `Data` or `None`.
    pub fn remove_asset(&self, full_path: &Path) -> Option<Data> {
        #[cfg(all(
            feature = "file_watcher",
            not(target_arch = "wasm32"),
            not(target_os = "android")
        ))]
        self.root_paths
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(full_path);

        self.dir.remove_asset(full_path)
    }

    /// Registers the [`EMBEDDED`] [`AssetSource`] to the given [`AssetSourceBuilders`].
    ///
    /// This function is automaticlly called by [`AssetPlugin`](crate::plugin::AssetPlugin);
    ///
    /// [`AssetSource`]: crate::io::AssetSource
    #[rustfmt::skip]
    pub fn register_source(&self, sources: &mut AssetSourceBuilders) {
        let dir = self.dir.clone();
        let p_dir = self.dir.clone();

        let reader_builder = move || {
            Box::new(MemoryAssetReader { root: dir.clone() }) as Box<dyn ErasedAssetReader>
        };
        let p_reader_builder = move || {
            Box::new(MemoryAssetReader { root: p_dir.clone() }) as Box<dyn ErasedAssetReader>
        };

        // Note that we only add a processed watch warning because we don't want to warn
        // noisily about embedded watching (which is niche) when users enable file watching.

        #[cfg(any(not(feature = "file_watcher"), target_arch = "wasm32", target_os = "android"))]
        let source = AssetSourceBuilder::new(reader_builder)
            .with_processed_reader(p_reader_builder)
            .with_processed_watch_warning("Consider enabling the `file_watcher` cargo feature.");

        #[cfg(all(feature = "file_watcher", not(target_arch = "wasm32"), not(target_os = "android")))]
        let source = {
            use crate::io::watcher::EmbeddedWatcher;
            use core::time::Duration;

            const DEBOUNCE: Duration = Duration::from_millis(300);

            let dir = self.dir.clone();
            let p_dir = self.dir.clone();
            let root_paths = self.root_paths.clone();
            let p_root_paths = self.root_paths.clone();

            let watcher_builder = move |sender| {
                EmbeddedWatcher::build(dir.clone(), root_paths.clone(), sender, DEBOUNCE)
            };

            let p_watcher_builder = move |sender| {
                EmbeddedWatcher::build(p_dir.clone(), p_root_paths.clone(), sender, DEBOUNCE)
            };

            AssetSourceBuilder::new(reader_builder)
                .with_processed_reader(p_reader_builder)
                .with_watcher(watcher_builder)
                .with_processed_watcher(p_watcher_builder)
                .with_processed_watch_warning("Platform not support EmbeddedWatcher.")
        };

        sources.insert(EMBEDDED, source);
    }
}
