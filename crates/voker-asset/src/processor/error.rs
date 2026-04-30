use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use thiserror::Error;
use voker_ecs::error::GameError;

use crate::io::{AssetReaderError, AssetWriterError};
use crate::meta::DeserializeMetaError;
use crate::processor::ValidateLogError;
use crate::server::{AssetLoadError, MissingAssetLoader};

/// Error encountered during [`AssetProcessor::process`].
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum AssetProcessError {
    #[error(transparent)]
    DeserializeMetaError(#[from] DeserializeMetaError),
    #[error(transparent)]
    MissingProcessor(#[from] MissingProcessor),
    #[error(transparent)]
    MissingAssetLoader(#[from] MissingAssetLoader),
    #[error("The wrong meta type was passed into a processor")]
    WrongMetaType,
    #[error("Assets without extensions are not supported")]
    ExtensionRequired,
    #[error("Encountered an error while loading the asset: {0}")]
    AssetLoadError(#[from] AssetLoadError),
    #[error("Encountered an error while transforming the asset: {0}")]
    AssetTransformError(GameError),
    #[error("Encountered an error while saving the asset: {0}")]
    AssetSaveError(GameError),
    #[error("Encountered an AssetReader error for '{path}': {err}")]
    AssetReaderError {
        path: Box<str>, // reduce 8 bytes (compared to String)
        err: AssetReaderError,
    },
    #[error("Encountered an AssetWriter error for '{path}': {err}")]
    AssetWriterError {
        path: Box<str>, // reduce 8 bytes (compared to String)
        err: AssetWriterError,
    },
}

#[derive(Error, Debug)]
pub enum InitializeError {
    #[error(transparent)]
    FailedToReadSourcePaths(AssetReaderError),
    #[error(transparent)]
    FailedToReadTargetPaths(AssetReaderError),
    #[error("Failed to validate asset log: {0}")]
    ValidateLogError(#[from] ValidateLogError),
}

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum SetTransactionLogFactoryError {
    #[error("Transaction log is already in use so setting the factory does nothing")]
    AlreadyInUse,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum MissingProcessor {
    #[error("The processor '{0}' does not exist")]
    Missing(String),
    #[error("The processor '{name}' is ambiguous between several processors: {ambiguous:?}")]
    Ambiguous {
        name: Box<str>, // reduce 8 bytes (compared to String)
        ambiguous: Vec<&'static str>,
    },
}
