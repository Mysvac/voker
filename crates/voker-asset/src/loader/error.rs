use std::path::PathBuf;

use alloc::sync::Arc;
use thiserror::Error;
use voker_ecs::error::GameError;

use crate::io::{AssetReaderError, MissingAssetSource, MissingProcessedAssetReader};
use crate::meta::DeserializeMetaError;
use crate::path::AssetPath;
use crate::server::AssetLoadError;

// -----------------------------------------------------------------------------
// AssetLoaderError

/// An error that can occur during asset loading.
#[derive(Error, Debug, Clone)]
#[error("Failed to load asset '{path}' with asset loader '{loader_name}': {error}")]
pub struct AssetLoaderError {
    pub path: AssetPath<'static>,
    pub loader_name: &'static str,
    pub error: Arc<GameError>,
}

// -----------------------------------------------------------------------------
// AssetLoaderError

/// An error that can occur during asset loading.
#[derive(Error, Debug, Clone)]
#[error("Failed to load asset '{path}', asset loader '{loader_name}' panicked")]
pub struct AssetLoaderPanic {
    pub path: AssetPath<'static>,
    pub loader_name: &'static str,
}

// -----------------------------------------------------------------------------
// ReadAssetBytesError

/// An error produced when calling [`LoadContext::read_asset_bytes`].
///
/// [`LoadContext::read_asset_bytes`]: crate::loader::LoadContext::read_asset_bytes
#[derive(Error, Debug)]
pub enum ReadAssetBytesError {
    #[error("Attempted to load an asset with an empty path `{0}`")]
    EmptyPath(AssetPath<'static>),
    #[error(transparent)]
    AssetReaderError(#[from] AssetReaderError),
    #[error(transparent)]
    DeserializeMetaError(#[from] DeserializeMetaError),
    #[error(transparent)]
    MissingAssetSource(#[from] MissingAssetSource),
    #[error(transparent)]
    MissingProcessedAssetReader(#[from] MissingProcessedAssetReader),
    #[error("LoadContext requires asset hash for '{0}', but none was provided")]
    MissingAssetHash(AssetPath<'static>),
    #[error("Encountered an io error while loading asset at `{}`: {error}", path.display())]
    Io {
        path: PathBuf,
        error: std::io::Error,
    },
}

// -----------------------------------------------------------------------------
// LoadDirectError

#[derive(Error, Debug)]
pub enum LoadDirectError {
    #[error("Attempted to load an asset with an empty path \"{0}\"")]
    EmptyPath(AssetPath<'static>),
    #[error(
        "Requested to load an asset path ({0:?}) with a subasset, but this is unsupported. See issue #18291"
    )]
    RequestedSubasset(AssetPath<'static>),
    #[error("Failed to load dependency {dependency:?} {error}")]
    LoadError {
        dependency: AssetPath<'static>,
        error: AssetLoadError,
    },
}
