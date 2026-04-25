use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::meta::{AssetHash, DeserializeMetaError};
use crate::path::AssetPath;

// -----------------------------------------------------------------------------
// ProcessedInfo

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessDependencyInfo {
    pub full_hash: AssetHash,
    pub path: AssetPath<'static>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessedInfo {
    pub hash: AssetHash,
    pub full_hash: AssetHash,
    pub process_dependencies: Vec<ProcessDependencyInfo>,
}

// -----------------------------------------------------------------------------
// AssetInfo

#[derive(Serialize, Deserialize)]
pub enum AssetConfig<LoaderSettings, ProcessSettings> {
    Load {
        loader: String,
        settings: LoaderSettings,
    },
    Process {
        processor: String,
        settings: ProcessSettings,
    },
    Ignore,
}

#[derive(Serialize, Deserialize)]
pub enum AssetConfigKind {
    Load { loader: String },
    Process { processor: String },
    Ignore,
}

// -----------------------------------------------------------------------------
// accelerator

#[derive(Deserialize)]
pub struct ProcessedInfoMinimal {
    pub processed_info: Option<ProcessedInfo>,
}

#[derive(Deserialize)]
pub struct AssetConfigMinimal {
    pub asset_config: AssetConfigKind,
}

impl ProcessedInfoMinimal {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DeserializeMetaError> {
        ron::de::from_bytes::<Self>(bytes).map_err(DeserializeMetaError::ProcessInfo)
    }
}

impl AssetConfigMinimal {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DeserializeMetaError> {
        ron::de::from_bytes::<Self>(bytes).map_err(DeserializeMetaError::AssetConfig)
    }
}
