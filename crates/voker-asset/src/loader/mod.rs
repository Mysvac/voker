mod builder;
mod context;
mod error;
mod loaded;
mod loaders;

pub use builder::*;
pub use context::*;
pub use error::*;
pub use loaded::*;

pub(crate) use loaded::AssetContainer;
pub(crate) use loaded::LabeledAsset;
pub(crate) use loaders::*;

// -----------------------------------------------------------------------------
// Inline

use alloc::boxed::Box;
use core::any::{Any, TypeId};

use serde::{Deserialize, Serialize};
use voker_ecs::error::GameError;
use voker_reflect::info::TypePath;

use crate::BoxedFuture;
use crate::asset::Asset;
use crate::io::Reader;
use crate::meta::{AssetConfig, AssetMeta, DeserializeMetaError, DynamicAssetMeta, Settings};

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
    fn extensions(&self) -> &[&'static str] {
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

    fn asset_type_path(&self) -> &'static str;

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
        Box::pin(async move {
            let settings = <dyn Any>::downcast_ref::<L::Settings>(settings)
                .expect("AssetLoader settings should match the loader type");

            let asset = <L as AssetLoader>::load(self, reader, settings, &mut load_context)
                .await
                .map_err(Into::into)?;

            Ok(load_context.finish(asset).into())
        })
    }

    fn extensions(&self) -> &[&'static str] {
        <L as AssetLoader>::extensions(self)
    }

    fn deserialize_meta(
        &self,
        meta: &[u8],
    ) -> Result<Box<dyn DynamicAssetMeta>, DeserializeMetaError> {
        Ok(Box::new(AssetMeta::<L, ()>::deserialize(meta)?))
    }

    fn default_meta(&self) -> Box<dyn DynamicAssetMeta> {
        let config = AssetConfig::Load {
            loader: self.type_path().into(),
            settings: L::Settings::default(),
        };

        Box::new(AssetMeta::<L, ()>::new(config))
    }

    fn type_path(&self) -> &'static str {
        <L as TypePath>::type_path()
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<L>()
    }

    fn asset_type_path(&self) -> &'static str {
        <L::Asset as TypePath>::type_path()
    }

    fn asset_type_id(&self) -> TypeId {
        TypeId::of::<L::Asset>()
    }
}

// --------------------------------------------------------------
// Placeholder

/// A placeholder, this loader should never be called.
///
/// This implementation exists to make the meta format nicer to work with.
impl AssetLoader for () {
    type Asset = ();
    type Settings = ();
    type Error = GameError;

    async fn load(
        &self,
        _reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        unreachable!()
    }

    fn extensions(&self) -> &[&'static str] {
        unreachable!()
    }
}
