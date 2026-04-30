mod context;
mod error;
mod info;
mod pipeline;
mod server;
mod transaction;

pub(crate) use server::ProcessingState;

pub use context::*;
pub use error::*;
pub use info::*;
pub use pipeline::*;
pub use server::*;
pub use transaction::*;

// -----------------------------------------------------------------------------
// Inline

use alloc::borrow::ToOwned;
use core::any::Any;

use alloc::boxed::Box;
use serde::{Deserialize, Serialize};
use voker_reflect::info::TypePath;

use crate::BoxedFuture;
use crate::io::Writer;
use crate::loader::AssetLoader;
use crate::meta::{AssetConfig, AssetMeta, DeserializeMetaError};
use crate::meta::{DynamicAssetMeta, MetaIdentKind, Settings};

// -----------------------------------------------------------------------------
// AssetProcessor

/// Low-level asset processing trait: reads source bytes via [`ProcessContext`], transforms them,
/// and writes processed bytes to `writer`.
///
/// For most cases, prefer [`LoadTransformAndSave`], which composes an [`AssetLoader`],
/// an [`AssetTransformer`], and an [`AssetSaver`] into a full pipeline.
///
/// [`AssetTransformer`]: crate::transformer::AssetTransformer
/// [`AssetSaver`]: crate::saver::AssetSaver
pub trait AssetProcessor: TypePath + Send + Sync + 'static {
    /// The [`AssetLoader`] used to load the final processed output.
    type Loader: AssetLoader;
    /// Per-asset processor settings stored in the asset meta file.
    type Settings: Settings + Default + Serialize + for<'a> Deserialize<'a>;

    /// Processes the source asset and writes the result to `writer`.
    ///
    /// Returns the [`AssetLoader::Settings`] required to load the processed output.
    fn process(
        &self,
        writer: &mut dyn Writer,
        settings: &Self::Settings,
        context: &mut ProcessContext<'_>,
    ) -> impl Future<Output = Result<<Self::Loader as AssetLoader>::Settings, AssetProcessError>> + Send;
}

// -----------------------------------------------------------------------------
// ErasedAssetProcessor

/// A type-erased variant of [`AssetProcessor`].
pub trait ErasedAssetProcessor: Send + Sync + 'static {
    /// Type-erased variant of [`AssetProcessor::process`].
    ///
    /// Takes `context` by value to avoid the invariant dual-lifetime problem that arises when
    /// passing `&'a mut ProcessContext<'a>` — the caller can extract mutably-borrowed fields
    /// back from `context.new_processed_info` after this future resolves.
    fn process<'a>(
        &'a self,
        writer: &'a mut dyn Writer,
        settings: &'a dyn Settings,
        context: ProcessContext<'a>,
    ) -> BoxedFuture<'a, Result<Box<dyn DynamicAssetMeta>, AssetProcessError>>;

    /// Deserializes `meta` as a type-erased [`AssetMeta`] for this processor.
    fn deserialize_meta(
        &self,
        meta: &[u8],
    ) -> Result<Box<dyn DynamicAssetMeta>, DeserializeMetaError>;

    /// Returns the fully-qualified type path of the underlying [`AssetProcessor`].
    fn type_path(&self) -> &'static str;

    /// Returns the short type name of the underlying [`AssetProcessor`].
    fn type_name(&self) -> &'static str;

    /// Returns the default type-erased [`AssetMeta`] for this processor, using either
    /// the full type path or short type name according to `path_kind`.
    fn default_meta(&self, ident_kind: MetaIdentKind) -> Box<dyn DynamicAssetMeta>;
}

impl<T: AssetProcessor> ErasedAssetProcessor for T {
    fn process<'a>(
        &'a self,
        writer: &'a mut dyn Writer,
        settings: &'a dyn Settings,
        mut context: ProcessContext<'a>,
    ) -> BoxedFuture<'a, Result<Box<dyn DynamicAssetMeta>, AssetProcessError>> {
        Box::pin(async move {
            let settings = <dyn Any>::downcast_ref::<T::Settings>(settings)
                .ok_or(AssetProcessError::WrongMetaType)?;

            let settings = AssetProcessor::process(self, writer, settings, &mut context).await?;
            let loader = <T::Loader as TypePath>::type_path().to_owned();
            let config = AssetConfig::Load { loader, settings };

            Ok(Box::new(AssetMeta::<T::Loader, ()>::new(config)) as Box<dyn DynamicAssetMeta>)
        })
    }

    fn deserialize_meta(
        &self,
        meta: &[u8],
    ) -> Result<Box<dyn DynamicAssetMeta>, DeserializeMetaError> {
        Ok(Box::new(AssetMeta::<(), T>::deserialize(meta)?))
    }

    fn type_path(&self) -> &'static str {
        <T as TypePath>::type_path()
    }

    fn type_name(&self) -> &'static str {
        <T as TypePath>::type_name()
    }

    fn default_meta(&self, kind: MetaIdentKind) -> Box<dyn DynamicAssetMeta> {
        let processor = match kind {
            MetaIdentKind::TypePath => <T as TypePath>::type_path().into(),
            MetaIdentKind::TypeName => <T as TypePath>::type_name().into(),
        };

        Box::new(AssetMeta::<(), T>::new(AssetConfig::Process {
            processor,
            settings: T::Settings::default(),
        }))
    }
}

// -----------------------------------------------------------------------------
// Placeholder

/// A placeholder [`AssetProcessor`] that should never be called.
///
/// This exists solely to satisfy the type parameter in [`AssetMeta`] when no
/// processor is configured.
impl AssetProcessor for () {
    type Loader = ();
    type Settings = ();

    async fn process(
        &self,
        _writer: &mut dyn Writer,
        _settings: &Self::Settings,
        _context: &mut ProcessContext<'_>,
    ) -> Result<<Self::Loader as AssetLoader>::Settings, AssetProcessError> {
        unreachable!()
    }
}
