use core::marker::PhantomData;

use serde::{Deserialize, Serialize};
use voker_reflect::info::TypePath;

use super::{AssetProcessError, AssetProcessor, ProcessContext};
use crate::io::Writer;
use crate::loader::AssetLoader;
use crate::saver::{AssetSaver, SavedAsset};
use crate::transformer::{AssetTransformer, IdentityAssetTransformer, TransformedAsset};

// -----------------------------------------------------------------------------
// LoadTransformAndSave

/// A high-level [`AssetProcessor`] implementation that:
/// 1. Loads the source asset with loader `L`,
/// 2. Transforms it using transformer `T`,
/// 3. Saves the result using saver `S`.
///
/// Use [`IdentityAssetTransformer`] as `T` for direct load-then-save (format conversion).
#[derive(TypePath)]
#[type_path = "voker_asset::processor::LoadTransformAndSave"]
pub struct LoadTransformAndSave<L, T, S>
where
    L: AssetLoader,
    T: AssetTransformer<AssetInput = L::Asset>,
    S: AssetSaver<Asset = T::AssetOutput>,
{
    transformer: T,
    saver: S,
    marker: PhantomData<fn() -> L>,
}

// -----------------------------------------------------------------------------
// LoadTransformAndSave

impl<L, S> From<S> for LoadTransformAndSave<L, IdentityAssetTransformer<L::Asset>, S>
where
    L: AssetLoader,
    S: AssetSaver<Asset = L::Asset>,
{
    fn from(saver: S) -> Self {
        LoadTransformAndSave {
            transformer: IdentityAssetTransformer::new(),
            saver,
            marker: PhantomData,
        }
    }
}

impl<L, T, S> LoadTransformAndSave<L, T, S>
where
    L: AssetLoader,
    T: AssetTransformer<AssetInput = L::Asset>,
    S: AssetSaver<Asset = T::AssetOutput>,
{
    /// Creates a new `LoadTransformAndSave` with the given transformer and saver.
    pub fn new(transformer: T, saver: S) -> Self {
        LoadTransformAndSave {
            transformer,
            saver,
            marker: PhantomData,
        }
    }
}

// -----------------------------------------------------------------------------
// LoadTransformAndSaveSettings

/// Settings for [`LoadTransformAndSave`].
#[derive(Serialize, Deserialize, Default)]
pub struct LoadTransformAndSaveSettings<LoaderSettings, TransformerSettings, SaverSettings> {
    /// Settings forwarded to the [`AssetLoader`].
    pub loader_settings: LoaderSettings,
    /// Settings forwarded to the [`AssetTransformer`].
    pub transformer_settings: TransformerSettings,
    /// Settings forwarded to the [`AssetSaver`].
    pub saver_settings: SaverSettings,
}

// -----------------------------------------------------------------------------
// AssetProcessor impl for LoadTransformAndSave

impl<L, T, S> AssetProcessor for LoadTransformAndSave<L, T, S>
where
    L: AssetLoader,
    T: AssetTransformer<AssetInput = L::Asset>,
    S: AssetSaver<Asset = T::AssetOutput>,
{
    type Loader = S::Loader;
    type Settings = LoadTransformAndSaveSettings<L::Settings, T::Settings, S::Settings>;

    async fn process(
        &self,
        writer: &mut dyn Writer,
        settings: &Self::Settings,
        context: &mut ProcessContext<'_>,
    ) -> Result<<Self::Loader as AssetLoader>::Settings, AssetProcessError> {
        let loaded = context.load_source_asset::<L>(&settings.loader_settings).await?;

        let pre_transformed = TransformedAsset::<L::Asset>::from_loaded(loaded)
            .expect("load_source_asset should return an asset of the loader's type");

        let post_transformed = self
            .transformer
            .transform(pre_transformed, &settings.transformer_settings)
            .await
            .map_err(|e| AssetProcessError::AssetTransformError(e.into()))?;

        let saved_asset = SavedAsset::<T::AssetOutput>::from_transformed(&post_transformed);

        let output_settings = self
            .saver
            .save(
                saved_asset,
                writer,
                &settings.saver_settings,
                context.path().clone(),
            )
            .await
            .map_err(|e| AssetProcessError::AssetSaveError(e.into()))?;

        Ok(output_settings)
    }
}
