mod builder;
mod context;
mod loaded;

pub use builder::*;
pub use context::*;
pub use loaded::*;

// -----------------------------------------------------------------------------
// Inline

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::TypeId;

use serde::{Deserialize, Serialize};
use voker_ecs::error::GameError;
use voker_reflect::info::TypePath;

use crate::BoxedFuture;
use crate::asset::Asset;
use crate::handle::ErasedHandle;
use crate::io::Reader;
use crate::meta::{DeserializeMetaError, DynamicAssetMeta, Settings};

// --------------------------------------------------------------
// LoadedFolder

pub struct LoadedFolder {
    pub handles: Vec<ErasedHandle>,
}

// --------------------------------------------------------------
// AssetLoader

pub trait AssetLoader: TypePath + Send + Sync + 'static {
    type Asset: Asset;
    type Settings: Settings + Default + Serialize + for<'a> Deserialize<'a>;
    type Error: Into<GameError>;

    fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        load_context: &mut LoadContext,
    ) -> impl Future<Output = Result<Self::Asset, Self::Error>> + Send;

    #[inline(always)]
    fn extensions(&self) -> &[&str] {
        &[]
    }
}

pub trait ErasedAssetLoader: Send + Sync + 'static {
    fn load<'a>(
        &'a self,
        reader: &'a mut dyn Reader,
        settings: &'a dyn Settings,
        load_context: LoadContext<'a>,
    ) -> BoxedFuture<'a, Result<ErasedLoadedAsset, GameError>>;

    fn extensions(&self) -> &[&str];

    fn deserialize_meta(
        &self,
        meta: &[u8],
    ) -> Result<Box<dyn DynamicAssetMeta>, DeserializeMetaError>;

    fn default_meta(&self) -> Box<dyn DynamicAssetMeta>;

    fn type_path(&self) -> &'static str;

    fn type_id(&self) -> TypeId;

    fn asset_type_name(&self) -> &'static str;

    fn asset_type_id(&self) -> TypeId;
}

impl<L: AssetLoader> ErasedAssetLoader for L {
    /// Processes the asset in an asynchronous closure.
    fn load<'a>(
        &'a self,
        reader: &'a mut dyn Reader,
        settings: &'a dyn Settings,
        mut load_context: LoadContext<'a>,
    ) -> BoxedFuture<'a, Result<ErasedLoadedAsset, GameError>> {
        todo!()
    }

    fn extensions(&self) -> &[&str] {
        <L as AssetLoader>::extensions(self)
    }

    fn deserialize_meta(
        &self,
        meta: &[u8],
    ) -> Result<Box<dyn DynamicAssetMeta>, DeserializeMetaError> {
        todo!()
    }

    fn default_meta(&self) -> Box<dyn DynamicAssetMeta> {
        todo!()
    }

    fn type_path(&self) -> &'static str {
        L::type_path()
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<L>()
    }

    fn asset_type_name(&self) -> &'static str {
        core::any::type_name::<L::Asset>()
    }

    fn asset_type_id(&self) -> TypeId {
        TypeId::of::<L::Asset>()
    }
}
