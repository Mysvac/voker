use alloc::boxed::Box;
use std::path::{Path, PathBuf};

use voker_ecs::derive::Resource;

use crate::io::{AssetSourceBuilder, AssetSourceBuilders, Data};
use crate::io::{Dir, EMBEDDED, ErasedAssetReader, MemoryAssetReader, Value};

#[cfg(all(feature = "file_watcher", not(target_arch = "wasm32")))]
use voker_os::Arc;
#[cfg(all(feature = "file_watcher", not(target_arch = "wasm32")))]
use voker_os::sync::{PoisonError, RwLock};
#[cfg(all(feature = "file_watcher", not(target_arch = "wasm32")))]
use voker_utils::hash::HashMap;

#[derive(Resource, Default)]
pub struct EmbeddedAssetRegistry {
    dir: Dir,
    #[cfg(all(feature = "file_watcher", not(target_arch = "wasm32")))]
    root_paths: Arc<RwLock<HashMap<Box<Path>, PathBuf>>>,
}

impl EmbeddedAssetRegistry {
    pub fn insert_asset(&self, full_path: PathBuf, asset_path: &Path, value: impl Into<Value>) {
        #[cfg(not(all(feature = "file_watcher", not(target_arch = "wasm32"))))]
        let _ = full_path;
        #[cfg(all(feature = "file_watcher", not(target_arch = "wasm32")))]
        self.root_paths
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(full_path.into(), asset_path.to_path_buf());

        self.dir.insert_asset(asset_path, value);
    }

    pub fn insert_meta(&self, full_path: &Path, asset_path: &Path, value: impl Into<Value>) {
        #[cfg(not(all(feature = "file_watcher", not(target_arch = "wasm32"))))]
        let _ = full_path;
        #[cfg(all(feature = "file_watcher", not(target_arch = "wasm32")))]
        self.root_paths
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(full_path.into(), asset_path.to_path_buf());

        self.dir.insert_meta(asset_path, value);
    }

    pub fn remove_asset(&self, full_path: &Path) -> Option<Data> {
        self.dir.remove_asset(full_path)
    }

    #[rustfmt::skip]
    pub fn register_source(&self, sources: &mut AssetSourceBuilders) {
        let dir = self.dir.clone();
        let p_dir = self.dir.clone();

        let reader_builder =
            move || Box::new(MemoryAssetReader { root: dir.clone() }) as Box<dyn ErasedAssetReader>;
        let p_reader_builder = move || {
            Box::new(MemoryAssetReader {
                root: p_dir.clone(),
            }) as Box<dyn ErasedAssetReader>
        };

        // Note that we only add a processed watch warning because we don't want to warn
        // noisily about embedded watching (which is niche) when users enable file watching.

        #[cfg(not(all(feature = "file_watcher", not(target_arch = "wasm32"))))]
        let source = AssetSourceBuilder::new(reader_builder)
            .with_processed_reader(p_reader_builder)
            .with_processed_watch_warning("Consider enabling the `file_watcher` cargo feature.");

        #[cfg(all(feature = "file_watcher", not(target_arch = "wasm32")))]
        let source = {
            use crate::io::EmbeddedWatcher;
            use core::time::Duration;

            const DEBOUNCE_WAIT_TIME: Duration = Duration::from_millis(300);

            let dir = self.dir.clone();
            let p_dir = self.dir.clone();
            let root_paths = self.root_paths.clone();
            let p_root_paths = self.root_paths.clone();

            let watcher_builder = move |sender| {
                EmbeddedWatcher::new(dir.clone(), root_paths.clone(), sender, DEBOUNCE_WAIT_TIME)
            };

            let p_watcher_builder = move |sender| {
                EmbeddedWatcher::new(
                    p_dir.clone(),
                    p_root_paths.clone(),
                    sender,
                    DEBOUNCE_WAIT_TIME,
                )
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
