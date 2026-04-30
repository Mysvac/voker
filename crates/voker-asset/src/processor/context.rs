use alloc::boxed::Box;

use voker_reflect::info::TypePath;

use super::AssetProcessError;
use crate::io::Reader;
use crate::loader::{AssetLoader, ErasedLoadedAsset};
use crate::meta::{ProcessDependencyInfo, ProcessedInfo};
use crate::path::AssetPath;
use crate::server::AssetServer;

// -----------------------------------------------------------------------------
// ProcessContext

/// Context passed to [`AssetProcessor::process`], providing access to the source asset data.
///
/// [`AssetProcessor::process`]: crate::processor::AssetProcessor::process
pub struct ProcessContext<'a> {
    pub(crate) server: &'a AssetServer,
    /// Accumulates process dependencies discovered during processing.
    pub(crate) new_processed_info: &'a mut ProcessedInfo,
    pub(crate) path: &'a AssetPath<'static>,
    pub(crate) reader: Box<dyn Reader + 'a>,
}

impl<'a> ProcessContext<'a> {
    pub(crate) fn new(
        server: &'a AssetServer,
        path: &'a AssetPath<'static>,
        reader: Box<dyn Reader + 'a>,
        new_processed_info: &'a mut ProcessedInfo,
    ) -> Self {
        Self {
            server,
            new_processed_info,
            path,
            reader,
        }
    }

    /// Loads the source asset using loader `L` with the given settings.
    ///
    /// Any load dependencies are recorded as process dependencies.
    pub async fn load_source_asset<L: AssetLoader>(
        &mut self,
        settings: &L::Settings,
    ) -> Result<ErasedLoadedAsset, AssetProcessError> {
        let loader = self
            .server
            .get_asset_loader_by_path(<L as TypePath>::type_path())
            .await
            .map_err(AssetProcessError::MissingAssetLoader)?;

        let loaded_asset = self
            .server
            .load_with_loader(self.path, settings, &*loader, &mut *self.reader, true, true)
            .await
            .map_err(AssetProcessError::AssetLoadError)?;

        for (path, &full_hash) in &loaded_asset.loader_dependencies {
            self.new_processed_info
                .process_dependencies
                .push(ProcessDependencyInfo {
                    path: path.clone(),
                    full_hash,
                });
        }

        Ok(loaded_asset)
    }

    /// Returns the path of the asset being processed.
    #[inline]
    pub fn path(&self) -> &AssetPath<'static> {
        self.path
    }

    /// Returns a mutable reference to the raw asset reader.
    #[inline]
    pub fn asset_reader(&mut self) -> &mut dyn Reader {
        &mut *self.reader
    }
}
