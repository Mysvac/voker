use alloc::boxed::Box;
use core::any::TypeId;

use alloc::string::String;
use alloc::vec::Vec;

use alloc::sync::Arc;
use thiserror::Error;

use crate::io::AssetReaderError;
use crate::io::AssetWriterError;
use crate::io::MissingAssetSource;
use crate::io::MissingAssetWriter;
use crate::io::MissingProcessedAssetReader;
use crate::loader::AssetLoaderError;
use crate::loader::AssetLoaderPanic;
use crate::meta::DeserializeMetaError;
use crate::path::AssetPath;

// -----------------------------------------------------------------------------
// AssetLoadError

#[derive(Error, Debug, Clone)]
#[non_exhaustive]
pub enum AssetLoadError {
    #[error(transparent)]
    AddAsyncError(#[from] AddAsyncError),
    #[error(transparent)]
    AssetReaderError(#[from] AssetReaderError),
    #[error(transparent)]
    AssetLoaderError(#[from] AssetLoaderError),
    #[error(transparent)]
    AssetLoaderPanic(#[from] AssetLoaderPanic),
    #[error(transparent)]
    MissingAssetSource(#[from] MissingAssetSource),
    #[error(transparent)]
    MissingAssetLoader(#[from] MissingAssetLoader),
    #[error(transparent)]
    MissingLabeledAsset(#[from] MissingLabeledAsset),
    #[error(transparent)]
    MissingAssetLoaderFull(#[from] MissingAssetLoaderFull),
    #[error(transparent)]
    MissingProcessedAssetReader(#[from] MissingProcessedAssetReader),
    #[error(transparent)]
    RequestedHandleTypeMismatch(#[from] RequestedHandleTypeMismatch),
    #[error("Attempted to load an asset with an empty path \"{0}\"")]
    EmptyPath(AssetPath<'static>),
    #[error("Encountered an error while reading asset metadata bytes for `{0}`.")]
    AssetMetaReadError(AssetPath<'static>),
    #[error("Asset '{0}' is configured to be ignored. It cannot be loaded.")]
    CannotLoadIgnoredAsset(AssetPath<'static>),
    #[error("Asset '{0}' is configured to be processed. It cannot be loaded directly.")]
    CannotLoadProcessedAsset(AssetPath<'static>),
    #[error("Failed to deserialize meta for asset {path}: {error}")]
    DeserializeMetaError {
        path: AssetPath<'static>,
        error: Box<DeserializeMetaError>,
    },
}

// -----------------------------------------------------------------------------
// AddAsyncError

#[derive(Error, Debug, Clone)]
#[error("An error occurred while resolving an asset added by `add_async`: {error}")]
pub struct AddAsyncError {
    pub error: Arc<dyn core::error::Error + Send + Sync + 'static>,
}

// -----------------------------------------------------------------------------
// MissingHandleProvider

#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("Cannot allocate a handle because no handle provider exists for asset type {0:?}")]
pub struct MissingHandleProvider(pub TypeId);

// -----------------------------------------------------------------------------
// RequestedHandleTypeMismatch

#[derive(Error, Debug, Clone)]
#[error(
    "Requested handle of type {requested:?} for asset '{path}' does not \
    match actual asset type '{asset_path}', which used loader '{loader_path}'"
)]
pub struct RequestedHandleTypeMismatch {
    // Use String instead of AssetPath to reduce type size.
    pub path: String,
    pub requested: TypeId,
    pub asset_path: &'static str,
    pub loader_path: &'static str,
}

// -----------------------------------------------------------------------------
// MissingAssetLoader

#[derive(Error, Debug, Clone)]
pub enum MissingAssetLoader {
    #[error("no `AssetLoader` found with the type path '{0}'")]
    TypePath(String),
    #[error("no `AssetLoader` found with the type name '{0}'")]
    TypeName(String),
    #[error("no `AssetLoader` found with the asset TypeId '{0:?}'")]
    AssetType(TypeId),
    #[error("no `AssetLoader` found  for the asset path: {0}")]
    AssetPath(AssetPath<'static>),
    #[error("no `AssetLoader` found  for the following extensions: {0:?}")]
    Extension(Vec<String>),
}

#[derive(Error, Debug, Clone)]
#[error(
    "Could not find an asset loader matching: \
    Loader Path: {loader_path:?}; \
    Loader Name: {loader_name:?}; \
    Asset Type: {asset_type_id:?}; \
    Extension: {extension:?}; \
    Asset Path: {asset_path:?};"
)]
pub struct MissingAssetLoaderFull {
    /// use `Box<str>` instead of String to reduce type size.
    pub loader_path: Option<Box<str>>,
    pub loader_name: Option<Box<str>>,
    pub asset_type_id: Option<TypeId>,
    pub extension: Option<Box<str>>,
    pub asset_path: Option<Box<str>>,
}

// -----------------------------------------------------------------------------
// MissingLabeledAsset

#[derive(Error, Debug, Clone)]
#[error(
    "The file at '{base_path}' does not contain the labeled asset '{label}'; it contains the following assets: {all_labels:?}"
)]
pub struct MissingLabeledAsset {
    pub base_path: String,
    pub label: String,
    pub all_labels: Vec<String>,
}

// -----------------------------------------------------------------------------
// WaitForAssetError

#[derive(Error, Debug, Clone)]
pub enum WaitForAssetError {
    /// The asset is not being loaded; waiting for it is meaningless.
    #[error("tried to wait for an asset that is not being loaded")]
    NotLoaded,
    /// The asset failed to load.
    #[error(transparent)]
    Failed(Arc<AssetLoadError>),
    /// A dependency of the asset failed to load.
    #[error(transparent)]
    DependencyFailed(Arc<AssetLoadError>),
}

// -----------------------------------------------------------------------------
// WaitForAssetError

#[derive(Error, Debug)]
pub enum WriteDefaultMetaError {
    #[error("asset meta file already exists, so avoiding overwrite")]
    MetaAlreadyExists,
    #[error(transparent)]
    MissingAssetLoader(#[from] MissingAssetLoader),
    #[error(transparent)]
    MissingAssetSource(#[from] MissingAssetSource),
    #[error(transparent)]
    MissingAssetWriter(#[from] MissingAssetWriter),
    #[error("failed to write default asset meta file: {0}")]
    FailedToWriteMeta(#[from] AssetWriterError),
    #[error("encountered an I/O error while reading the existing meta file: {0}")]
    IoErrorFromExistingMetaCheck(Arc<std::io::Error>),
    #[error("encountered HTTP status {0} when reading the existing meta file")]
    HttpErrorFromExistingMetaCheck(u16),
}
