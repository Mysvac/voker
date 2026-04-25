use alloc::borrow::ToOwned;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::path::PathBuf;

use voker_utils::hash::{HashMap, HashSet};

use super::AssetProcessError;
use crate::ident::AssetSourceId;
use crate::io::AssetReaderError;
use crate::meta::{AssetHash, ProcessedInfo};
use crate::path::AssetPath;
use crate::server::AssetLoadError;

// -----------------------------------------------------------------------------
// ProcessStatus & ProcessResult

#[derive(Debug, Clone)]
pub enum ProcessResult {
    Processed(ProcessedInfo),
    SkippedNotChanged,
    Ignored,
}

/// The final status of processing an asset
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum ProcessStatus {
    Processed,
    Failed,
    NonExistent,
}

// -----------------------------------------------------------------------------
// ProcessorAssetInfo

#[derive(Debug)]
pub(crate) struct ProcessorAssetInfo {
    pub(crate) processed_info: Option<ProcessedInfo>,
    /// Paths of assets that depend on this asset when they are being processed.
    pub(crate) dependents: HashSet<AssetPath<'static>>,
    pub(crate) status: Option<ProcessStatus>,
    /// A lock that controls read/write access to processed asset files. The lock is shared for both the asset bytes and the meta bytes.
    /// _This lock must be locked whenever a read or write to processed assets occurs_
    /// There are scenarios where processed assets (and their metadata) are being read and written in multiple places at once:
    /// * when the processor is running in parallel with an app
    /// * when processing assets in parallel, the processor might read an asset's `process_dependencies` when processing new versions of those dependencies
    ///     * this second scenario almost certainly isn't possible with the current implementation, but its worth protecting against
    ///
    /// This lock defends against those scenarios by ensuring readers don't read while processed files are being written. And it ensures
    /// Because this lock is shared across meta and asset bytes, readers can ensure they don't read "old" versions of metadata with "new" asset data.
    pub(crate) file_transaction_lock: Arc<async_lock::RwLock<()>>,
    pub(crate) status_sender: async_broadcast::Sender<ProcessStatus>,
    pub(crate) status_receiver: async_broadcast::Receiver<ProcessStatus>,
}

impl Default for ProcessorAssetInfo {
    fn default() -> Self {
        let (mut status_sender, status_receiver) = async_broadcast::broadcast(1);
        // allow overflow on these "one slot" channels to allow receivers to retrieve the "latest" state,
        // and to allow senders to not block if there was older state present.
        status_sender.set_overflow(true);
        Self {
            processed_info: Default::default(),
            dependents: Default::default(),
            file_transaction_lock: Default::default(),
            status: None,
            status_sender,
            status_receiver,
        }
    }
}

impl ProcessorAssetInfo {
    async fn update_status(&mut self, status: ProcessStatus) {
        if self.status != Some(status) {
            self.status = Some(status);
            self.status_sender.broadcast(status).await.unwrap();
        }
    }
}

// -----------------------------------------------------------------------------
// ProcessorAssetInfos

#[derive(Default, Debug)]
pub struct ProcessorAssetInfos {
    /// The "current" in memory view of the asset space. During processing, if path does not exist in this, it should
    /// be considered non-existent.
    /// NOTE: YOU MUST USE `Self::get_or_insert` or `Self::insert` TO ADD ITEMS TO THIS COLLECTION TO ENSURE
    /// `non_existent_dependents` DATA IS CONSUMED
    infos: HashMap<AssetPath<'static>, ProcessorAssetInfo>,
    /// Dependents for assets that don't exist. This exists to track "dangling" asset references due to deleted / missing files.
    /// If the dependent asset is added, it can "resolve" these dependencies and re-compute those assets.
    /// Therefore this _must_ always be consistent with the `infos` data. If a new asset is added to `infos`, it should
    /// check this maps for dependencies and add them. If an asset is removed, it should update the dependents here.
    non_existent_dependents: HashMap<AssetPath<'static>, HashSet<AssetPath<'static>>>,
}

impl ProcessorAssetInfos {
    #[cfg(feature = "file_watcher")]
    pub(crate) fn paths(&self) -> impl Iterator<Item = &AssetPath<'static>> {
        self.infos.keys()
    }

    pub(crate) fn get_or_insert(
        &mut self,
        asset_path: AssetPath<'static>,
    ) -> &mut ProcessorAssetInfo {
        self.infos.entry(asset_path.clone()).or_insert_with(|| {
            let mut info = ProcessorAssetInfo::default();
            // track existing dependents by resolving existing "hanging" dependents.
            if let Some(dependents) = self.non_existent_dependents.remove(&asset_path) {
                info.dependents = dependents;
            }
            info
        })
    }

    pub(crate) fn get(&self, asset_path: &AssetPath<'static>) -> Option<&ProcessorAssetInfo> {
        self.infos.get(asset_path)
    }

    pub(crate) fn get_mut(
        &mut self,
        asset_path: &AssetPath<'static>,
    ) -> Option<&mut ProcessorAssetInfo> {
        self.infos.get_mut(asset_path)
    }

    pub(crate) fn add_dependent(
        &mut self,
        asset_path: &AssetPath<'static>,
        dependent: AssetPath<'static>,
    ) {
        if let Some(info) = self.get_mut(asset_path) {
            info.dependents.insert(dependent);
        } else {
            let entry = self.non_existent_dependents.entry(asset_path.clone());
            entry.or_default().insert(dependent);
        }
    }

    pub(crate) fn remove_dependencies(
        &mut self,
        asset_path: &AssetPath<'static>,
        removed_info: ProcessedInfo,
    ) {
        for old_load_dep in removed_info.process_dependencies {
            let path = &old_load_dep.path;
            if let Some(info) = self.infos.get_mut(path) {
                info.dependents.remove(asset_path);
            } else if let Some(dependents) = self.non_existent_dependents.get_mut(path) {
                dependents.remove(asset_path);
            }
        }
    }

    #[cfg(feature = "file_watcher")]
    pub(crate) async fn remove(
        &mut self,
        asset_path: &AssetPath<'static>,
    ) -> Option<Arc<async_lock::RwLock<()>>> {
        let info = self.infos.remove(asset_path)?;

        if let Some(processed_info) = info.processed_info {
            self.remove_dependencies(asset_path, processed_info);
        }

        // Tell all listeners this asset does not exist
        info.status_sender
            .broadcast(ProcessStatus::NonExistent)
            .await
            .unwrap();

        if !info.dependents.is_empty() {
            tracing::error!(
                "The asset at {asset_path} was removed, but it had assets that depend on it to be processed. \
                Consider updating the path in the following assets: {:?}",
                info.dependents
            );
            self.non_existent_dependents
                .insert(asset_path.clone(), info.dependents);
        }

        Some(info.file_transaction_lock)
    }

    #[cfg(feature = "file_watcher")]
    pub(crate) async fn rename(
        &mut self,
        old: &AssetPath<'static>,
        new: &AssetPath<'static>,
        new_task_sender: &async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
    ) -> Option<(Arc<async_lock::RwLock<()>>, Arc<async_lock::RwLock<()>>)> {
        let mut info = self.infos.remove(old)?;

        if !info.dependents.is_empty() {
            // TODO: We can't currently ensure "moved" folders with relative paths aren't broken because AssetPath
            // doesn't distinguish between absolute and relative paths. We have "erased" relativeness. In the short term,
            // we could do "remove everything in a folder and re-add", but that requires full rebuilds / destroying the cache.
            // If processors / loaders could enumerate dependencies, we could check if the new deps line up with a rename.
            // If deps encoded "relativeness" as part of loading, that would also work (this seems like the right call).
            // TODO: it would be nice to log an error here for dependents that aren't also being moved + fixed.
            // (see the remove impl).
            tracing::error!(
                "The asset at {old} was removed, but it had assets that depend on it to be processed. \
                Consider updating the path in the following assets: {:?}",
                info.dependents
            );
            let dependents = core::mem::take(&mut info.dependents);
            self.non_existent_dependents.insert(old.clone(), dependents);
        }

        if let Some(processed_info) = &info.processed_info {
            // Update "dependent" lists for this asset's "process dependencies" to use new path.
            for dep in &processed_info.process_dependencies {
                if let Some(info) = self.infos.get_mut(&dep.path) {
                    info.dependents.remove(old);
                    info.dependents.insert(new.clone());
                } else if let Some(dependents) = self.non_existent_dependents.get_mut(&dep.path) {
                    dependents.remove(old);
                    dependents.insert(new.clone());
                }
            }
        }

        // Tell all listeners this asset no longer exists
        info.status_sender
            .broadcast(ProcessStatus::NonExistent)
            .await
            .unwrap();

        let new_info = self.get_or_insert(new.clone());
        new_info.processed_info = info.processed_info;
        new_info.status = info.status;

        // Ensure things waiting on the new path are informed of the status of this asset
        if let Some(status) = new_info.status {
            new_info.status_sender.broadcast(status).await.unwrap();
        }

        let dependents = new_info.dependents.iter().cloned().collect::<Vec<_>>();

        // Queue the asset for a reprocess check, in case it needs new meta.
        let _ = new_task_sender
            .send((new.source().clone_owned(), new.path().to_owned()))
            .await;

        for dependent in dependents {
            // Queue dependents for reprocessing because they might have been waiting for this asset.
            let _ = new_task_sender
                .send((
                    dependent.source().clone_owned(),
                    dependent.path().to_owned(),
                ))
                .await;
        }

        Some((
            info.file_transaction_lock,
            new_info.file_transaction_lock.clone(),
        ))
    }

    pub(crate) async fn finish_processing(
        &mut self,
        asset_path: AssetPath<'static>,
        result: Result<ProcessResult, AssetProcessError>,
        reprocess_sender: async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
    ) {
        match result {
            Ok(ProcessResult::Processed(processed_info)) => {
                tracing::debug!("Finished processing \"{}\"", asset_path);
                // clean up old dependents
                let old_processed_info =
                    self.infos.get_mut(&asset_path).and_then(|i| i.processed_info.take());
                if let Some(old_processed_info) = old_processed_info {
                    self.remove_dependencies(&asset_path, old_processed_info);
                }

                // populate new dependents
                for process_dependency_info in &processed_info.process_dependencies {
                    self.add_dependent(&process_dependency_info.path, asset_path.to_owned());
                }

                let info = self.get_or_insert(asset_path);
                info.processed_info = Some(processed_info);
                info.update_status(ProcessStatus::Processed).await;

                let dependents = info.dependents.iter().cloned().collect::<Vec<_>>();
                for path in dependents {
                    let _ = reprocess_sender
                        .send((path.source().clone_owned(), path.path().to_owned()))
                        .await;
                }
            }
            Ok(ProcessResult::SkippedNotChanged) => {
                tracing::debug!("Skipping processing (unchanged) \"{}\"", asset_path);
                let info = self.get_mut(&asset_path).expect("info should exist");
                // NOTE: skipping an asset on a given pass doesn't mean it won't change in the future as a result
                // of a dependency being re-processed. This means apps might receive an "old" (but valid) asset first.
                // This is in the interest of fast startup times that don't block for all assets being checked + reprocessed
                // Therefore this relies on hot-reloading in the app to pickup the "latest" version of the asset
                // If "block until latest state is reflected" is required, we can easily add a less granular
                // "block until first pass finished" mode
                info.update_status(ProcessStatus::Processed).await;
            }
            Ok(ProcessResult::Ignored) => {
                tracing::debug!("Skipping processing (ignored) \"{}\"", asset_path);
            }
            Err(AssetProcessError::ExtensionRequired) => {
                // Skip assets without extensions
            }
            Err(AssetProcessError::MissingAssetLoader(e)) => {
                tracing::trace!("No loader found for {asset_path}: {e}");
            }
            Err(AssetProcessError::AssetReaderError {
                err: AssetReaderError::NotFound(_),
                ..
            }) => {
                // if there is no asset source, no processing can be done
                tracing::trace!("No need to process asset {asset_path} because it does not exist");
            }
            Err(AssetProcessError::AssetLoadError(AssetLoadError::AssetLoaderError(err))) => {
                tracing::error!("Failed to load asset {asset_path}: {err}");
                self.add_dependent(&err.path, asset_path.to_owned());
                let info = self.get_mut(&asset_path).expect("info should exist");
                info.processed_info = Some(ProcessedInfo {
                    hash: AssetHash::ZERO,
                    full_hash: AssetHash::ZERO,
                    process_dependencies: Vec::new(),
                });
                info.update_status(ProcessStatus::Failed).await;
            }
            Err(err) => {
                tracing::error!("Failed to process asset {asset_path}: {err}");
                let info = self.get_mut(&asset_path).expect("info should exist");
                info.update_status(ProcessStatus::Failed).await;
            }
        }
    }
}
