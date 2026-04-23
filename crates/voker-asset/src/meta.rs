use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;

use ron::de::SpannedError;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use voker_ecs::error::GameError;

use crate::path::AssetPath;

// -----------------------------------------------------------------------------
// META_FORMAT_VERSION

pub const META_FORMAT_VERSION: &str = "1.0";

// -----------------------------------------------------------------------------
// ProcessedInfo

pub type AssetHash = [u8; 32];

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessDependencyInfo {
    pub full_hash: AssetHash,
    pub path: AssetPath<'static>,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct ProcessedInfo {
    pub hash: AssetHash,
    pub full_hash: AssetHash,
    pub process_dependencies: Vec<ProcessDependencyInfo>,
}

#[derive(Serialize, Deserialize)]
pub struct ProcessedInfoMinimal {
    pub processed_info: Option<ProcessedInfo>,
}

// -----------------------------------------------------------------------------
// AssetAction

#[derive(Serialize, Deserialize)]
pub enum AssetAction<LoaderSettings, ProcessSettings> {
    /// Load the asset with the given loader and settings
    /// See [`AssetLoader`].
    Load {
        loader: String,
        settings: LoaderSettings,
    },
    /// Process the asset with the given processor and settings.
    /// See [`Process`] and [`AssetProcessor`].
    ///
    /// [`AssetProcessor`]: crate::processor::AssetProcessor
    Process {
        processor: String,
        settings: ProcessSettings,
    },
    /// Do nothing with the asset
    Ignore,
}

#[derive(Serialize, Deserialize)]
pub enum AssetActionMinimal {
    Load { loader: String },
    Process { processor: String },
    Ignore,
}

// -----------------------------------------------------------------------------
// Settings

pub trait Settings: Any + Send + Sync {}

impl<T: Send + Sync + Any> Settings for T {}

// -----------------------------------------------------------------------------
// AssetAction

pub trait DynamicAssetMeta: Any + Send + Sync {
    /// Returns a reference to the [`AssetLoader`] settings, if they exist.
    fn loader_settings(&self) -> Option<&dyn Settings>;
    /// Returns a mutable reference to the [`AssetLoader`] settings, if they exist.
    fn loader_settings_mut(&mut self) -> Option<&mut dyn Settings>;
    /// Returns a reference to the [`Process`] settings, if they exist.
    fn process_settings(&self) -> Option<&dyn Settings>;
    /// Serializes the internal [`AssetMeta`].
    fn serialize(&self) -> Vec<u8>;
    /// Returns a reference to the [`ProcessedInfo`] if it exists.
    fn processed_info(&self) -> &Option<ProcessedInfo>;
    /// Returns a mutable reference to the [`ProcessedInfo`] if it exists.
    fn processed_info_mut(&mut self) -> &mut Option<ProcessedInfo>;
}

pub type MetaTransform = Box<dyn Fn(&mut dyn DynamicAssetMeta) + Send + Sync>;

// -----------------------------------------------------------------------------
// AssetAction

/// An error that occurs while deserializing [`AssetMeta`].
#[derive(GameError, Error, Debug, Clone, PartialEq, Eq)]
#[game_error(severity = "error")]
pub enum DeserializeMetaError {
    #[error("Failed to deserialize asset meta: {0:?}")]
    DeserializeSettings(#[from] SpannedError),
    #[error("Failed to deserialize minimal asset meta: {0:?}")]
    DeserializeMinimal(SpannedError),
}
