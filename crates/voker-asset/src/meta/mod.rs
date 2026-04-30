mod hash;
mod info;
mod version;

pub use hash::AssetHash;
pub use info::*;
pub use version::*;

// -----------------------------------------------------------------------------
// Inline

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::Any;
use ron::de::SpannedError;
use thiserror::Error;

use crate::loader::AssetLoader;
use crate::processor::AssetProcessor;
use serde::{Deserialize, Serialize};

// ------------------------------------------------------------
// Settings

pub trait Settings: Any + Send + Sync {}

impl<T: Send + Sync + Any> Settings for T {}

// ------------------------------------------------------------
// MetaTransform

pub type MetaTransform = Box<dyn Fn(&mut dyn DynamicAssetMeta) + Send + Sync>;

// ------------------------------------------------------------
// AssetMeta

#[derive(Serialize, Deserialize)]
pub struct AssetMeta<L: AssetLoader, P: AssetProcessor> {
    // Currently only one version, no need to customize serializer and deserializer.
    #[serde(default)]
    pub format_version: FormatVersion,
    pub asset_config: AssetConfig<L::Settings, P::Settings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed_info: Option<ProcessedInfo>,
}

impl<L: AssetLoader, P: AssetProcessor> AssetMeta<L, P> {
    pub fn new(config: AssetConfig<L::Settings, P::Settings>) -> Self {
        Self {
            asset_config: config,
            format_version: FormatVersion::default(),
            processed_info: None,
        }
    }

    /// Deserializes the given serialized byte representation of the asset meta.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, DeserializeMetaError> {
        Ok(ron::de::from_bytes(bytes)?)
    }

    pub fn serialize(&self) -> Vec<u8> {
        use ron::ser::PrettyConfig;
        // This defaults to \r\n on Windows, hard-code it to \n for consistent.
        let config = PrettyConfig::default().new_line("\n");
        ron::ser::to_string_pretty(&self, config)
            .expect("type is convertible to ron")
            .into_bytes()
    }
}

/// An error that occurs while deserializing [`AssetMeta`].
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum DeserializeMetaError {
    #[error("Failed to deserialize asset meta: {0:?}")]
    Normal(#[from] SpannedError),
    #[error("Failed to deserialize minimal asset config: {0:?}")]
    AssetConfig(SpannedError),
    #[error("Failed to deserialize minimal process info: {0:?}")]
    ProcessInfo(SpannedError),
}

// ------------------------------------------------------------
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

impl<L: AssetLoader, P: AssetProcessor> DynamicAssetMeta for AssetMeta<L, P> {
    fn loader_settings(&self) -> Option<&dyn Settings> {
        match &self.asset_config {
            AssetConfig::Load { settings, .. } => Some(settings),
            _ => None,
        }
    }

    fn loader_settings_mut(&mut self) -> Option<&mut dyn Settings> {
        match &mut self.asset_config {
            AssetConfig::Load { settings, .. } => Some(settings),
            _ => None,
        }
    }

    fn process_settings(&self) -> Option<&dyn Settings> {
        match &self.asset_config {
            AssetConfig::Process { settings, .. } => Some(settings),
            _ => None,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        <AssetMeta<L, P>>::serialize(self)
    }

    fn processed_info(&self) -> &Option<ProcessedInfo> {
        &self.processed_info
    }

    fn processed_info_mut(&mut self) -> &mut Option<ProcessedInfo> {
        &mut self.processed_info
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetaIdentKind {
    #[default]
    TypePath,
    TypeName,
}
