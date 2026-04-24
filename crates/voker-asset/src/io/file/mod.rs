

voker_task::cfg::single_threaded! {
    mod sync_asset;
}

voker_task::cfg::multi_threaded! {
    mod async_asset;
}

// -----------------------------------------------------------------------------
// base_path

use std::env;
use std::path::{Path, PathBuf};

pub(crate) fn base_path() -> PathBuf {
    if let Ok(manifest_dir) = env::var("VOKER_ASSET_ROOT") {
        PathBuf::from(manifest_dir)
    } else if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        PathBuf::from(manifest_dir)
    } else {
        env::current_exe().unwrap()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }
}

// -----------------------------------------------------------------------------
// FileAssetReader

#[derive(Debug, Clone)]
pub struct FileAssetReader {
    root_path: PathBuf,
}

impl FileAssetReader {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let root_path = base_path().join(path.as_ref());
        tracing::debug!(
            "Asset Server using {} as its base path.",
            root_path.display()
        );
        Self { root_path }
    }

    pub fn base_path() -> PathBuf {
        base_path()
    }

    pub fn root_path(&self) -> &PathBuf {
        &self.root_path
    }
}

#[derive(Debug, Clone)]
pub struct FileAssetWriter {
    root_path: PathBuf,
}

impl FileAssetWriter {
    pub fn new<P: AsRef<Path>>(path: P, create_root: bool) -> Self {
        let root_path = base_path().join(path.as_ref());
        if create_root && let Err(e) = std::fs::create_dir_all(&root_path) {
            tracing::error!(
                "Failed to create root directory {} for file asset writer: {e}",
                root_path.display(),
            );
        }
        Self { root_path }
    }

    pub fn base_path() -> PathBuf {
        base_path()
    }

    pub fn root_path(&self) -> &PathBuf {
        &self.root_path
    }
}


