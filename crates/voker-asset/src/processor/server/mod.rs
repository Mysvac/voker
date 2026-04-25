mod internal;
use internal::*;

pub(crate) use internal::ProcessingState;

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::path::PathBuf;

use futures_lite::StreamExt;
use voker_ecs::borrow::Res;
use voker_ecs::derive::Resource;
use voker_os::sync;
use voker_os::sync::PoisonError;
use voker_reflect::info::TypePath;
use voker_task::IoTaskPool;

use crate::ident::AssetSourceId;
#[cfg(feature = "file_watcher")]
use crate::io::AssetSourceEvent;
use crate::io::future::WriteAllFuture;
use crate::io::{
    AssetReaderError, AssetSource, AssetSourceBuilders, AssetSources, AssetWriterError,
    MissingAssetSource, SliceReader, Writer,
};
use crate::meta::{AssetConfigKind, AssetConfigMinimal};
use crate::meta::{AssetHash, ProcessedInfo, ProcessedInfoMinimal};
use crate::path::AssetPath;
use crate::processor::{AssetProcessError, AssetProcessor, ErasedAssetProcessor};
use crate::processor::{InitializeError, MissingProcessor, ProcessContext, ProcessResult};
use crate::processor::{ProcessStatus, validate_transaction_log};
use crate::processor::{SetTransactionLogFactoryError, TransactionLogFactory};
use crate::server::{AssetServer, AssetServerMode, MetaCheckMode, UnapprovedPathMode};

// -----------------------------------------------------------------------------
// AssetProcessServer

#[derive(Resource, Clone)]
pub struct AssetProcessServer {
    pub(crate) server: AssetServer,
    pub(crate) data: Arc<AssetProcessorData>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ProcessorState {
    /// The processor is still initializing, which involves scanning the current asset folders,
    /// constructing an in-memory view of the asset space, recovering from previous errors / crashes,
    /// and cleaning up old / unused assets.
    Initializing,
    /// The processor is currently processing assets.
    Processing,
    /// The processor has finished processing all valid assets and reporting invalid assets.
    Finished,
}

// -----------------------------------------------------------------------------
// VecWriter (internal)

/// A [`Writer`] that collects written bytes into a [`Vec<u8>`].
struct VecWriter {
    bytes: Vec<u8>,
}

impl VecWriter {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl futures_io::AsyncWrite for VecWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.get_mut().bytes.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl Writer for VecWriter {
    #[inline]
    fn write_all_bytes<'a>(&'a mut self, buf: &'a [u8]) -> WriteAllFuture<'a> {
        WriteAllFuture::vec_write(&mut self.bytes, buf)
    }
}

// -----------------------------------------------------------------------------
// Hash utility (internal)

/// Computes a 256-bit non-cryptographic hash of two byte slices.
///
/// Uses four parallel FNV-1a streams with different seeds for independence.
fn hash_asset_data(data1: &[u8], data2: &[u8]) -> AssetHash {
    const PRIME: u64 = 0x0000_0100_0000_01B3;

    let mut h0: u64 = 0xcbf2_9ce4_8422_2325;
    let mut h1: u64 = 0xcbf2_9ce4_8422_2326;
    let mut h2: u64 = 0xcbf2_9ce4_8422_2327;
    let mut h3: u64 = 0xcbf2_9ce4_8422_2328;

    for &b in data1.iter().chain(data2.iter()) {
        h0 ^= b as u64;
        h0 = h0.wrapping_mul(PRIME);
        h1 ^= b.wrapping_add(1) as u64;
        h1 = h1.wrapping_mul(PRIME);
        h2 ^= b.wrapping_add(2) as u64;
        h2 = h2.wrapping_mul(PRIME);
        h3 ^= b.wrapping_add(3) as u64;
        h3 = h3.wrapping_mul(PRIME);
    }

    let mut result = [0u8; 32];
    result[0..8].copy_from_slice(&h0.to_le_bytes());
    result[8..16].copy_from_slice(&h1.to_le_bytes());
    result[16..24].copy_from_slice(&h2.to_le_bytes());
    result[24..32].copy_from_slice(&h3.to_le_bytes());
    AssetHash(result)
}

// -----------------------------------------------------------------------------
// AssetProcessServer Implementation

impl AssetProcessServer {
    /// Creates a new [`AssetProcessServer`] backed by the given [`AssetServer`].
    #[must_use]
    pub fn build(
        builders: &mut AssetSourceBuilders,
        watch_processed: bool,
        transaction_log_factory: Option<Box<dyn TransactionLogFactory>>,
    ) -> (Self, Arc<AssetSources>) {
        let state = Arc::new(ProcessingState::new());
        let mut sources = builders.build_sources(true, watch_processed);
        sources.gate_on_processor(state.clone());
        let sources = Arc::new(sources);

        let data = Arc::new(AssetProcessorData {
            state,
            sources: sources.clone(),
            processors: sync::RwLock::new(Processors::default()),
            log_factory: sync::Mutex::new(transaction_log_factory),
            log: async_lock::RwLock::new(None),
        });

        let server = AssetServer::new(
            sources.clone(),
            AssetServerMode::Processed,
            MetaCheckMode::Always,
            false,
            UnapprovedPathMode::default(),
        );

        (Self { server, data }, sources)
    }

    /// The "internal" [`AssetServer`] used by the [`AssetProcessor`].
    ///
    /// This is _separate_ from the asset processor used by the main App.
    /// It has different processor-specific configuration and a different ID space.
    #[inline]
    pub fn server(&self) -> &AssetServer {
        &self.server
    }

    /// Retrieves the current [`ProcessorState`]
    #[inline]
    pub async fn get_state(&self) -> ProcessorState {
        self.data.state.get_state().await
    }

    #[inline]
    pub fn get_source<'a>(
        &self,
        id: impl Into<AssetSourceId<'a>>,
    ) -> Result<&AssetSource, MissingAssetSource> {
        self.data.sources.get(id.into())
    }

    #[inline]
    pub fn sources(&self) -> &AssetSources {
        &self.data.sources
    }

    // -------------------------------------------------------------------------
    // Configuration

    /// Overrides the transaction log factory.
    ///
    /// Returns an error if the log is already in use (i.e., [`run`](Self::run) was called).
    pub fn set_transaction_log_factory(
        &self,
        factory: Box<dyn TransactionLogFactory>,
    ) -> Result<(), SetTransactionLogFactoryError> {
        let mut guard = self.data.log_factory.lock().unwrap_or_else(PoisonError::into_inner);
        if guard.is_none() {
            // factory has already been taken by run() — the log is in use
            return Err(SetTransactionLogFactoryError::AlreadyInUse);
        }
        *guard = Some(factory);
        Ok(())
    }

    /// Returns a future that will not finish until the path has been processed.
    pub async fn wait_until_processed(&self, path: AssetPath<'static>) -> ProcessStatus {
        self.data.state.wait_until_processed(path).await
    }

    /// Returns a future that will not finish until the processor has been initialized.
    pub async fn wait_until_initialized(&self) {
        self.data.state.wait_until_initialized().await;
    }

    /// Returns a future that will not finish until processing has finished.
    pub async fn wait_until_finished(&self) {
        self.data.state.wait_until_finished().await;
    }

    /// Registers an [`AssetProcessor`] so it can be found by type path and type name.
    pub fn register_processor<P: AssetProcessor>(&self, processor: P) {
        self.data
            .processors
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Arc::new(processor));
    }

    /// Sets `P` as the default processor for assets with the given file `extension`.
    pub fn set_default_processor<P: AssetProcessor>(&self, extension: &str) {
        self.data
            .processors
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .set_default_processor(extension, <P as TypePath>::type_path());
    }

    // -------------------------------------------------------------------------
    // Queries

    /// Returns the processor registered for the given fully-qualified type path.
    pub fn get_processor_by_path(&self, type_path: &str) -> Option<Arc<dyn ErasedAssetProcessor>> {
        self.data
            .processors
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(type_path)
    }

    /// Returns the processor registered for the given short type name.
    ///
    /// Returns `Err` if the name is ambiguous.
    pub fn get_processor_by_name(
        &self,
        type_name: &str,
    ) -> Result<Option<Arc<dyn ErasedAssetProcessor>>, MissingProcessor> {
        self.data
            .processors
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get_by_name(type_name)
    }

    // -------------------------------------------------------------------------
    // Run loop

    /// Starts the asset processor as a background task on the [`IoTaskPool`].
    ///
    /// This is an ECS system intended to be added to the `Startup` schedule via
    /// `app.add_systems(Startup, AssetProcessServer::start)`.
    pub fn start(processor: Res<AssetProcessServer>) {
        let processor = processor.clone();
        IoTaskPool::get()
            .spawn(async move {
                tracing::debug!("Asset processor started");
                processor.run().await;
                tracing::debug!("Asset processor finished");
            })
            .detach();
    }

    /// Runs the full asset-processing pipeline:
    /// 1. Validates and sets up the transaction log.
    /// 2. Scans all asset sources and builds in-memory asset state.
    /// 3. Processes every asset that needs (re)processing.
    /// 4. Continues to process cascading dependent assets until stable.
    pub async fn run(&self) {
        if self.data.state.get_state().await != ProcessorState::Initializing {
            return;
        }

        // --- Transaction log ---
        let log_factory: Option<Box<dyn TransactionLogFactory>> = {
            let mut guard = self.data.log_factory.lock().unwrap_or_else(PoisonError::into_inner);
            guard.take()
        };

        if let Some(factory) = &log_factory
            && let Err(err) = validate_transaction_log(factory.as_ref()).await
        {
            tracing::error!(
                "Asset processor transaction log is invalid ({err}). \
                    All assets will be reprocessed."
            );
        }

        if let Some(factory) = log_factory {
            match factory.new_log().await {
                Ok(log) => *self.data.log.write().await = Some(log),
                Err(err) => tracing::error!("Failed to create transaction log: {err}"),
            }
        } else {
            tracing::warn!(
                "No transaction log factory configured. \
                 Processing will not be crash-recoverable."
            );
        }

        // --- Initialization ---
        // Use a channel so that process-dependency cascades can reuse the same queue.
        let (task_tx, task_rx) = async_channel::unbounded::<(AssetSourceId<'static>, PathBuf)>();

        match self.initialize(task_tx.clone()).await {
            Ok(()) => {}
            Err(err) => {
                tracing::error!("AssetProcessServer failed to initialize: {err}");
                self.data.state.set_state(ProcessorState::Finished).await;
                return;
            }
        }

        self.data.state.set_state(ProcessorState::Processing).await;

        // Collect the initial task list
        let mut pending: Vec<AssetPath<'static>> = {
            let mut v = Vec::new();
            while let Ok((source_id, path)) = task_rx.try_recv() {
                v.push(AssetPath::from(path).with_source(source_id));
            }
            v
        };
        drop(task_rx);

        // Process waves until no new dependents are queued
        loop {
            if pending.is_empty() {
                break;
            }

            let (reprocess_tx, reprocess_rx) =
                async_channel::unbounded::<(AssetSourceId<'static>, PathBuf)>();

            for asset_path in pending {
                let result = self.process_asset(&asset_path).await;
                let mut infos = self.data.state.asset_infos.write().await;
                infos
                    .finish_processing(asset_path, result, reprocess_tx.clone())
                    .await;
            }

            // Clean up AssetInfos entries for handles dropped during processing.
            // The processor's server uses independent handle providers (not shared with
            // Assets<A>), so track_assets never drains these drop events.
            self.server.write_infos().process_handle_drop_events();

            // Collect everything that was queued for reprocessing
            drop(reprocess_tx);
            let mut next = Vec::new();
            while let Ok((source_id, path)) = reprocess_rx.recv().await {
                next.push(AssetPath::from(path).with_source(source_id));
            }
            pending = next;
        }

        self.data.state.set_state(ProcessorState::Finished).await;

        #[cfg(feature = "file_watcher")]
        {
            let (watch_tx, watch_rx) =
                async_channel::unbounded::<(AssetSourceId<'static>, PathBuf)>();
            self.watch_for_changes(watch_tx, watch_rx).await;
        }
    }

    // -------------------------------------------------------------------------
    // Internal: watch_for_changes

    /// Processes file-system change events from all watched sources and queues
    /// assets for (re)processing. Runs until all event receivers are closed.
    #[cfg(feature = "file_watcher")]
    async fn watch_for_changes(
        &self,
        task_tx: async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
        task_rx: async_channel::Receiver<(AssetSourceId<'static>, PathBuf)>,
    ) {
        // Collect (source_id, receiver) pairs for all watched sources.
        let mut streams: Vec<(
            AssetSourceId<'static>,
            async_channel::Receiver<AssetSourceEvent>,
        )> = Vec::new();

        for source in self.data.sources.iter() {
            if let Some(rx) = source.event_receiver() {
                streams.push((source.id().clone_owned(), rx.clone()));
            }
        }

        if streams.is_empty() {
            return;
        }

        loop {
            // Drain queued reprocess tasks first.
            while let Ok((source_id, path)) = task_rx.try_recv() {
                let asset_path = AssetPath::from(path).with_source(source_id);
                let result = self.process_asset(&asset_path).await;
                let mut infos = self.data.state.asset_infos.write().await;
                infos.finish_processing(asset_path, result, task_tx.clone()).await;
            }

            let mut got_event = false;
            let mut closed_indices: Vec<usize> = Vec::new();

            for (idx, (source_id, rx)) in streams.iter().enumerate() {
                match rx.try_recv() {
                    Ok(event) => {
                        got_event = true;
                        self.handle_source_event(source_id.clone(), event, &task_tx).await;
                    }
                    Err(async_channel::TryRecvError::Empty) => {}
                    Err(async_channel::TryRecvError::Closed) => {
                        closed_indices.push(idx);
                    }
                }
            }

            for idx in closed_indices.into_iter().rev() {
                streams.remove(idx);
            }
            if streams.is_empty() {
                break;
            }
            if !got_event {
                futures_lite::future::yield_now().await;
            }
        }
    }

    /// Handles a single [`AssetSourceEvent`] from a watched source.
    #[cfg(feature = "file_watcher")]
    async fn handle_source_event(
        &self,
        source_id: AssetSourceId<'static>,
        event: AssetSourceEvent,
        task_tx: &async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
    ) {
        match event {
            AssetSourceEvent::AddedAsset(path)
            | AssetSourceEvent::ModifiedAsset(path)
            | AssetSourceEvent::AddedMeta(path)
            | AssetSourceEvent::ModifiedMeta(path) => {
                let _ = task_tx.send((source_id, path)).await;
            }
            AssetSourceEvent::RemovedAsset(path) | AssetSourceEvent::RemovedMeta(path) => {
                let asset_path = AssetPath::from(path).with_source(source_id);
                let mut infos = self.data.state.asset_infos.write().await;
                let _lock = infos.remove(&asset_path).await;
            }
            AssetSourceEvent::RenamedAsset { old, new }
            | AssetSourceEvent::RenamedMeta { old, new } => {
                let old_path = AssetPath::from(old).with_source(source_id.clone());
                let new_path = AssetPath::from(new).with_source(source_id);
                let mut infos = self.data.state.asset_infos.write().await;
                let _locks = infos.rename(&old_path, &new_path, task_tx).await;
            }
            AssetSourceEvent::RemovedUnknown { path, is_meta } => {
                if is_meta {
                    // Meta removed — reprocess with default meta / default processor.
                    let _ = task_tx.send((source_id, path)).await;
                } else {
                    let asset_path = AssetPath::from(path).with_source(source_id);
                    let mut infos = self.data.state.asset_infos.write().await;
                    let _lock = infos.remove(&asset_path).await;
                }
            }
            AssetSourceEvent::AddedFolder(path)
            | AssetSourceEvent::RenamedFolder { new: path, .. } => {
                if let Ok(source) = self.data.sources.get(source_id.clone())
                    && let Ok(paths) = scan_dir(source.reader(), &path).await
                {
                    for p in paths {
                        let _ = task_tx.send((source_id.clone(), p)).await;
                    }
                }
            }
            AssetSourceEvent::RemovedFolder(prefix) => {
                let mut infos = self.data.state.asset_infos.write().await;
                let to_remove: Vec<AssetPath<'static>> = infos
                    .paths()
                    .filter(|p| p.source() == &source_id && p.path().starts_with(&prefix))
                    .cloned()
                    .collect();
                for asset_path in to_remove {
                    let _lock = infos.remove(&asset_path).await;
                }
            }
        }
    }

    // -------------------------------------------------------------------------
    // Internal: initialize

    /// Scans all sources, builds [`ProcessorAssetInfos`], and queues every source
    /// asset for the initial processing pass.
    async fn initialize(
        &self,
        task_sender: async_channel::Sender<(AssetSourceId<'static>, PathBuf)>,
    ) -> Result<(), InitializeError> {
        for source in self.data.sources.iter() {
            if !source.should_process() {
                continue;
            }

            let source_id = source.id().clone_owned();
            let source_reader = source.reader();

            // Scan source directory
            let source_paths = scan_dir(source_reader, &PathBuf::new())
                .await
                .map_err(InitializeError::FailedToReadSourcePaths)?;

            // Scan processed directory for existing ProcessedInfo.
            // Must use the ungated reader here — the gated reader would block until
            // the processor reaches Processing state, but we're still in Initializing.
            let mut processed_with_info: Vec<(PathBuf, ProcessedInfo)> = Vec::new();
            if let Some(processed_reader) = source.ungated_processed_reader() {
                let processed_paths =
                    scan_dir(processed_reader, &PathBuf::new()).await.unwrap_or_default();

                for path in &processed_paths {
                    if let Ok(meta_bytes) = processed_reader.read_meta_bytes(path).await
                        && let Ok(minimal) = ProcessedInfoMinimal::from_bytes(&meta_bytes)
                        && let Some(info) = minimal.processed_info
                    {
                        processed_with_info.push((path.clone(), info));
                    }
                }
            }

            // Update in-memory asset state
            {
                let mut infos = self.data.state.asset_infos.write().await;

                for path in &source_paths {
                    let asset_path = AssetPath::from(path.clone()).with_source(source_id.clone());
                    infos.get_or_insert(asset_path);
                }

                for (path, processed_info) in processed_with_info {
                    let asset_path = AssetPath::from(path).with_source(source_id.clone());
                    if let Some(info) = infos.get_mut(&asset_path) {
                        info.processed_info = Some(processed_info);
                    }
                }
            }

            // Enqueue all source assets for processing
            for path in source_paths {
                let _ = task_sender.send((source_id.clone(), path)).await;
            }
        }

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Internal: process_asset

    /// Processes a single asset at `asset_path`.
    ///
    /// Returns:
    /// - `ProcessResult::Ignored` if the asset is excluded or has no processor.
    /// - `ProcessResult::SkippedNotChanged` if the source and all dependencies are unchanged.
    /// - `ProcessResult::Processed(info)` after a successful run.
    /// - `Err(...)` on any failure.
    async fn process_asset(
        &self,
        asset_path: &AssetPath<'static>,
    ) -> Result<ProcessResult, AssetProcessError> {
        let source = self.data.sources.get(asset_path.source()).map_err(|_| {
            AssetProcessError::AssetReaderError {
                path: asset_path.to_string().into(),
                err: AssetReaderError::NotFound(asset_path.path().to_owned()),
            }
        })?;

        if !source.should_process() {
            return Ok(ProcessResult::Ignored);
        }

        let path = asset_path.path();
        let source_reader = source.reader();

        // Read source meta to determine action; fall back to the default processor
        // for the file extension when no meta file exists.
        let meta_bytes_result = source_reader.read_meta_bytes(path).await;
        let (meta_bytes_opt, fallback_processor_opt): (
            Option<Vec<u8>>,
            Option<Arc<dyn ErasedAssetProcessor>>,
        ) = match meta_bytes_result {
            Ok(bytes) => (Some(bytes), None),
            Err(AssetReaderError::NotFound(_)) => {
                // No meta file — try the default processor for this extension.
                let extension = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
                let maybe_processor = self
                    .data
                    .processors
                    .read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .get_default_processor_by_extension(extension);
                (None, maybe_processor)
            }
            Err(err) => {
                return Err(AssetProcessError::AssetReaderError {
                    path: asset_path.to_string().into(),
                    err,
                });
            }
        };

        // If no meta and no default processor, there is nothing to do.
        if meta_bytes_opt.is_none() && fallback_processor_opt.is_none() {
            return Ok(ProcessResult::Ignored);
        }

        let config_minimal = if let Some(ref bytes) = meta_bytes_opt {
            AssetConfigMinimal::from_bytes(bytes)?
        } else {
            // No meta — treat as a Process action using the default processor.
            AssetConfigMinimal {
                asset_config: AssetConfigKind::Process {
                    processor: fallback_processor_opt.as_ref().unwrap().type_path().to_owned(),
                },
            }
        };

        match config_minimal.asset_config {
            AssetConfigKind::Ignore => Ok(ProcessResult::Ignored),
            // Load-configured assets are not processed; they are used as-is.
            AssetConfigKind::Load { .. } => Ok(ProcessResult::Ignored),
            AssetConfigKind::Process {
                processor: processor_key,
            } => {
                // Find the processor (try full path, then short name, then fallback)
                let processor: Arc<dyn ErasedAssetProcessor> = {
                    let procs = self.data.processors.read().unwrap_or_else(PoisonError::into_inner);
                    procs
                        .get(&processor_key)
                        .or_else(|| procs.get_by_name(&processor_key).ok().flatten())
                        .or(fallback_processor_opt)
                        .ok_or_else(|| {
                            AssetProcessError::MissingProcessor(MissingProcessor::Missing(
                                processor_key.clone(),
                            ))
                        })?
                };

                // Read source bytes for hashing
                let source_bytes = source_reader.read_bytes(path).await.map_err(|err| {
                    AssetProcessError::AssetReaderError {
                        path: asset_path.to_string().into(),
                        err,
                    }
                })?;

                let meta_bytes_slice: &[u8] = meta_bytes_opt.as_deref().unwrap_or(&[]);
                let source_hash = hash_asset_data(&source_bytes, meta_bytes_slice);

                // Check if the asset is unchanged (skip if so)
                if self.is_up_to_date(asset_path, source_hash).await {
                    return Ok(ProcessResult::SkippedNotChanged);
                }

                // Deserialize full meta to extract processor settings,
                // or use the default meta when no meta file exists.
                let meta = if let Some(ref bytes) = meta_bytes_opt {
                    processor.deserialize_meta(bytes)?
                } else {
                    processor.default_meta(crate::meta::MetaIdentKind::TypePath)
                };
                let settings = meta.process_settings().ok_or(AssetProcessError::WrongMetaType)?;

                // Acquire the per-asset write lock to prevent concurrent reads
                let _asset_write_lock = {
                    let infos = self.data.state.asset_infos.read().await;
                    infos.get(asset_path).map(|i| i.file_transaction_lock.clone())
                };
                let _guard = if let Some(lock) = _asset_write_lock {
                    Some(lock.write_arc().await)
                } else {
                    None
                };

                // Log begin
                self.log_begin(asset_path).await;

                // Build the process context
                let mut new_processed_info = ProcessedInfo {
                    hash: source_hash,
                    full_hash: source_hash,
                    process_dependencies: Vec::new(),
                };

                let mut output_writer = VecWriter::new();

                // context is taken by value so new_processed_info / output_writer
                // are freed immediately after the future resolves.
                let slice_reader = SliceReader::new(&source_bytes);
                let boxed_reader: Box<dyn crate::io::Reader + '_> = Box::new(slice_reader);
                let context = ProcessContext::new(
                    &self.server,
                    asset_path,
                    boxed_reader,
                    &mut new_processed_info,
                );

                let mut output_meta =
                    processor.process(&mut output_writer, settings, context).await?;

                let output_bytes = output_writer.into_bytes();

                // Compute full_hash: covers processed bytes + all dependency hashes
                let dep_hashes: Vec<u8> = new_processed_info
                    .process_dependencies
                    .iter()
                    .flat_map(|d| d.full_hash.0)
                    .collect();
                new_processed_info.full_hash = hash_asset_data(&output_bytes, &dep_hashes);

                // Embed ProcessedInfo into the output meta
                *output_meta.processed_info_mut() = Some(new_processed_info.clone());
                let serialized_meta = output_meta.serialize();

                // Write processed bytes and meta to the processed writer
                let processed_writer =
                    source
                        .processed_writer()
                        .map_err(|_| AssetProcessError::AssetWriterError {
                            path: asset_path.to_string().into(),
                            err: AssetWriterError::NotFound(path.to_owned()),
                        })?;

                processed_writer
                    .write_bytes(path, &output_bytes)
                    .await
                    .map_err(|err| AssetProcessError::AssetWriterError {
                        path: asset_path.to_string().into(),
                        err,
                    })?;

                processed_writer
                    .write_meta_bytes(path, &serialized_meta)
                    .await
                    .map_err(|err| AssetProcessError::AssetWriterError {
                        path: asset_path.to_string().into(),
                        err,
                    })?;

                // Log end
                self.log_end(asset_path).await;

                Ok(ProcessResult::Processed(new_processed_info))
            }
        }
    }

    // -------------------------------------------------------------------------
    // Internal helpers

    /// Returns `true` if the asset's stored hash matches `source_hash` and all
    /// process-dependency hashes are also still current.
    async fn is_up_to_date(&self, asset_path: &AssetPath<'static>, source_hash: AssetHash) -> bool {
        let infos = self.data.state.asset_infos.read().await;

        let Some(info) = infos.get(asset_path) else {
            return false;
        };
        let Some(processed_info) = &info.processed_info else {
            return false;
        };

        if processed_info.hash != source_hash {
            return false;
        }

        // Verify every process-dependency still has the expected full_hash
        for dep in &processed_info.process_dependencies {
            let dep_full_hash = infos
                .get(&dep.path)
                .and_then(|i| i.processed_info.as_ref())
                .map(|i| i.full_hash);
            if dep_full_hash != Some(dep.full_hash) {
                return false;
            }
        }

        true
    }

    async fn log_begin(&self, path: &AssetPath<'_>) {
        let mut log = self.data.log.write().await;
        if let Some(log) = log.as_mut()
            && let Err(err) = log.begin(path).await
        {
            tracing::error!("Transaction log begin error: {err}");
        }
    }

    async fn log_end(&self, path: &AssetPath<'_>) {
        let mut log = self.data.log.write().await;
        if let Some(log) = log.as_mut()
            && let Err(err) = log.end(path).await
        {
            tracing::error!("Transaction log end error: {err}");
        }
    }
}

// -----------------------------------------------------------------------------
// scan_dir helper

/// Recursively collects all file paths under `root` via `reader`.
///
/// Returns an empty list (rather than an error) when `root` does not exist.
async fn scan_dir(
    reader: &dyn crate::io::ErasedAssetReader,
    root: &std::path::Path,
) -> Result<Vec<PathBuf>, AssetReaderError> {
    let mut paths = Vec::new();
    let mut stack = alloc::vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        match reader.read_directory(&dir).await {
            Ok(mut stream) => {
                while let Some(child) = stream.next().await {
                    match reader.is_directory(&child).await {
                        Ok(true) => stack.push(child),
                        Ok(false) => paths.push(child),
                        Err(err) => return Err(err),
                    }
                }
            }
            Err(AssetReaderError::NotFound(_)) => {
                // root directory doesn't exist – that's fine
            }
            Err(err) => return Err(err),
        }
    }

    Ok(paths)
}
