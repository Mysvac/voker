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

/// A file-format plugin that deserialises raw bytes into an [`Asset`].
///
/// Implement this trait to support a new asset format.  The [`AssetServer`] selects
/// a loader for each load request by matching the path extension against the values
/// returned by [`extensions`](AssetLoader::extensions).
///
/// # Associated types
///
/// - `Asset` — the asset type produced by this loader.
/// - `Settings` — per-asset configuration stored in `.meta` sidecar files.
///   Must implement [`Default`] so it can be used when no `.meta` file exists.
/// - `Error` — the error type; converted to [`struct@GameError`] on failure.
///
/// # Example
///
/// ```rust
/// # use voker_asset::loader::{AssetLoader, LoadContext};
/// # use voker_asset::io::Reader;
/// # use voker_asset::meta::Settings;
/// # use voker_ecs::error::GameError;
/// # use voker_reflect::prelude::*;
/// # use serde::{Deserialize, Serialize};
/// # #[derive(Asset, Reflect)] struct MyText { pub content: String }
/// # #[derive(Default, Serialize, Deserialize)] struct TextSettings;
/// # impl Settings for TextSettings {}
/// #[derive(Default)]
/// struct TextLoader;
///
/// impl AssetLoader for TextLoader {
///     type Asset    = MyText;
///     type Settings = TextSettings;
///     type Error    = GameError;
///
///     async fn load(
///         &self,
///         reader:   &mut dyn Reader,
///         _settings: &TextSettings,
///         _ctx:     &mut LoadContext<'_>,
///     ) -> Result<MyText, GameError> {
///         let mut buf = String::new();
///         reader.read_to_string(&mut buf).await.map_err(GameError::new)?;
///         Ok(MyText { content: buf })
///     }
///
///     fn extensions(&self) -> &[&'static str] { &["txt"] }
/// }
/// ```
///
/// Register with [`AppAssetExt::register_asset_loader`](crate::plugin::AppAssetExt::register_asset_loader).
///
/// [`AssetServer`]: crate::server::AssetServer
pub trait AssetLoader: TypePath + Send + Sync + 'static {
    /// The asset type produced by this loader.
    type Asset: Asset;
    /// Per-asset configuration; stored in `.meta` files next to the asset.
    type Settings: Settings + Default + Serialize + for<'a> Deserialize<'a>;
    /// Error type returned when loading fails.
    type Error: Into<GameError>;

    /// Reads bytes from `reader` and returns the loaded asset.
    ///
    /// `settings` contains values from the asset's `.meta` sidecar (or defaults if
    /// no `.meta` exists).  `load_context` provides helpers for loading sub-assets
    /// and declaring dependencies.
    fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        load_context: &mut LoadContext,
    ) -> impl Future<Output = Result<Self::Asset, Self::Error>> + Send;

    /// File extensions handled by this loader (without leading `.`).
    ///
    /// Returns an empty slice by default, which means the loader must be selected
    /// explicitly (e.g. via a `.meta` file) rather than by extension matching.
    #[inline(always)]
    fn extensions(&self) -> &[&'static str] {
        &[]
    }
}

/// Object-safe, type-erased version of [`AssetLoader`] used internally by the asset server.
///
/// This trait is automatically implemented for every `T: AssetLoader` — implementors
/// should not implement it manually.
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
