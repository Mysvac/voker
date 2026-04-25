use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;
use core::time::Duration;

use alloc::sync::Arc;
use atomicow::CowArc;
use voker_ecs::derive::Resource;

use async_channel::{Receiver, Sender};
use voker_utils::hash::HashMap;

use super::AssetSourceEvent;
use super::{MissingAssetSource, MissingAssetWriter};
use super::{MissingProcessedAssetReader, MissingProcessedAssetWriter};
use crate::ident::AssetSourceId;
use crate::io::gated::ProcessorGatedReader;
use crate::io::watcher::AssetWatcher;
use crate::io::{ErasedAssetReader, ErasedAssetWriter};
use crate::processor::ProcessingState;

// -----------------------------------------------------------------------------
// AssetSourceBuilder & AssetSource

type ReaderBuilder = Box<dyn FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync>;
type WriterBuilder = Box<dyn FnMut() -> Option<Box<dyn ErasedAssetWriter>> + Send + Sync>;
type WatcherBuilder =
    Box<dyn FnMut(Sender<AssetSourceEvent>) -> Option<Box<dyn AssetWatcher>> + Send + Sync>;

/// Metadata about an "asset source".
///
/// Such as how to construct the [`AssetReader`] and [`AssetWriter`]
/// for the source, and whether or not the source is processed.
///
/// [`AssetReader`]: crate::io::AssetReader
/// [`AssetWriter`]: crate::io::AssetWriter
pub struct AssetSourceBuilder {
    /// The [`ErasedAssetReader`] to use on the unprocessed asset.
    pub reader: ReaderBuilder,

    /// The [`ErasedAssetWriter`] to use on the unprocessed asset.
    pub writer: Option<WriterBuilder>,

    /// The [`AssetWatcher`] to use for unprocessed assets, if any.
    pub watcher: Option<WatcherBuilder>,

    /// The [`ErasedAssetReader`] to use on the processed asset, if any.
    pub processed_reader: Option<ReaderBuilder>,

    /// The [`ErasedAssetWriter`] to use on the processed asset, if any.
    pub processed_writer: Option<WriterBuilder>,

    /// The [`AssetWatcher`] to use for processed assets, if any.
    pub processed_watcher: Option<WatcherBuilder>,

    /// The warning message to display when watching an unprocessed asset fails.
    pub watch_warning: Option<&'static str>,

    /// The warning message to display when watching a processed asset fails.
    pub processed_watch_warning: Option<&'static str>,
}

/// A collection of [`AssetReader`], [`AssetWriter`], and [`AssetWatcher`] instances.
///
/// For a specific asset source, identified by an [`AssetSourceId`].
///
/// [`AssetReader`]: crate::io::AssetReader
/// [`AssetWriter`]: crate::io::AssetWriter
pub struct AssetSource {
    id: AssetSourceId<'static>,
    reader: Box<dyn ErasedAssetReader>,
    writer: Option<Box<dyn ErasedAssetWriter>>,
    watcher: Option<Box<dyn AssetWatcher>>,
    processed_reader: Option<Arc<dyn ErasedAssetReader>>,
    processed_writer: Option<Box<dyn ErasedAssetWriter>>,
    processed_watcher: Option<Box<dyn AssetWatcher>>,
    event_receiver: Option<Receiver<AssetSourceEvent>>,
    processed_event_receiver: Option<Receiver<AssetSourceEvent>>,
    /// The ungated version of `processed_reader`.
    ///
    /// This allows the processor to read all the processed assets to initialize itself
    /// without being gated on itself (causing a deadlock).
    ungated_processed_reader: Option<Arc<dyn ErasedAssetReader>>,
}

// -----------------------------------------------------------------------------
// AssetSourceBuilder Implementation

impl AssetSourceBuilder {
    /// Creates a new builder, starting with the provided reader.
    #[inline]
    pub fn new(
        reader: impl FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync + 'static,
    ) -> AssetSourceBuilder {
        Self {
            reader: Box::new(reader),
            writer: None,
            watcher: None,
            processed_reader: None,
            processed_writer: None,
            processed_watcher: None,
            watch_warning: None,
            processed_watch_warning: None,
        }
    }

    /// Builds a new [`AssetSource`] with the given `id`.
    ///
    /// - If `watch` is true, the unprocessed source will watch for changes.
    /// - If `watch_processed` is true, the processed source will watch for changes.
    ///
    /// Note that the default watcher need `file_watcher` feature.
    pub fn build(
        &mut self,
        id: AssetSourceId<'static>,
        watch: bool,
        watch_processed: bool,
    ) -> AssetSource {
        let reader = self.reader.as_mut()();
        let writer = self.writer.as_mut().and_then(|w| w());
        let processed_reader = self.processed_reader.as_mut().map(|r| Arc::from(r()));
        let processed_writer = self.processed_writer.as_mut().and_then(|w| w());

        let mut source = AssetSource {
            id: id.clone(),
            reader,
            writer,
            processed_reader,
            processed_writer,
            watcher: None,
            event_receiver: None,
            processed_watcher: None,
            processed_event_receiver: None,
            ungated_processed_reader: None,
        };

        if watch {
            let (sender, receiver) = async_channel::unbounded();
            match self.watcher.as_mut().and_then(|w| w(sender)) {
                Some(w) => {
                    source.watcher = Some(w);
                    source.event_receiver = Some(receiver);
                }
                None => {
                    if let Some(warning) = self.watch_warning {
                        tracing::warn!("{id} does not have an AssetWatcher configured. {warning}");
                    }
                }
            }
        }

        if watch_processed {
            let (sender, receiver) = async_channel::unbounded();
            match self.processed_watcher.as_mut().and_then(|w| w(sender)) {
                Some(w) => {
                    source.processed_watcher = Some(w);
                    source.processed_event_receiver = Some(receiver);
                }
                None => {
                    if let Some(warning) = self.processed_watch_warning {
                        tracing::warn!(
                            "{id} does not have a processed AssetWatcher configured. {warning}"
                        );
                    }
                }
            }
        }

        source
    }

    /// Will use the given function to construct unprocessed [`ErasedAssetReader`].
    #[inline]
    pub fn with_reader(
        mut self,
        reader: impl FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync + 'static,
    ) -> Self {
        self.reader = Box::new(reader);
        self
    }

    /// Will use the given function to construct unprocessed [`ErasedAssetWriter`].
    #[inline]
    pub fn with_writer(
        mut self,
        writer: impl FnMut() -> Option<Box<dyn ErasedAssetWriter>> + Send + Sync + 'static,
    ) -> Self {
        self.writer = Some(Box::new(writer));
        self
    }

    /// Will use the given function to construct unprocessed [`AssetWatcher`].
    #[inline]
    pub fn with_watcher(
        mut self,
        watcher: impl FnMut(Sender<AssetSourceEvent>) -> Option<Box<dyn AssetWatcher>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.watcher = Some(Box::new(watcher));
        self
    }

    /// Will use the given function to construct processed [`ErasedAssetReader`].
    #[inline]
    pub fn with_processed_reader(
        mut self,
        reader: impl FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync + 'static,
    ) -> Self {
        self.processed_reader = Some(Box::new(reader));
        self
    }

    /// Will use the given function to construct processed [`ErasedAssetWriter`].
    #[inline]
    pub fn with_processed_writer(
        mut self,
        writer: impl FnMut() -> Option<Box<dyn ErasedAssetWriter>> + Send + Sync + 'static,
    ) -> Self {
        self.processed_writer = Some(Box::new(writer));
        self
    }

    /// Will use the given function to construct processed [`AssetWatcher`].
    #[inline]
    pub fn with_processed_watcher(
        mut self,
        watcher: impl FnMut(Sender<AssetSourceEvent>) -> Option<Box<dyn AssetWatcher>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.processed_watcher = Some(Box::new(watcher));
        self
    }

    /// Enables a warning for the unprocessed source watcher.
    ///
    /// which will print when watching is enabled and the unprocessed source doesn't have a watcher.
    #[inline]
    pub fn with_watch_warning(mut self, warning: &'static str) -> Self {
        self.watch_warning = Some(warning);
        self
    }

    /// Enables a warning for the processed source watcher.
    ///
    /// which will print when watching is enabled and the processed source doesn't have a watcher.
    #[inline]
    pub fn with_processed_watch_warning(mut self, warning: &'static str) -> Self {
        self.processed_watch_warning = Some(warning);
        self
    }

    /// Returns a builder containing the "platform default source" for the given `path` and `processed_path`.
    ///
    /// For most platforms, this will use [`FileAssetReader`] / [`FileAssetWriter`],
    /// but some platforms (such as Android and Wasm) have their own default readers / writers / watchers.
    ///
    /// [`FileAssetReader`]: crate::io::file::FileAssetReader
    /// [`FileAssetWriter`]: crate::io::file::FileAssetWriter
    #[rustfmt::skip]
    pub fn platform_default(path: &str, processed_path: Option<&str>) -> Self {
        const DEBOUNCE: Duration = Duration::from_millis(300);

        let default = Self::new(AssetSource::default_reader(path.to_owned(), false))
            .with_writer(AssetSource::default_writer(path.to_owned(), false))
            .with_watcher(AssetSource::default_watcher(path.to_owned(), false, DEBOUNCE))
            .with_watch_warning(AssetSource::default_watch_warning());

        let Some(p_path) = processed_path else {
            return default;
        };

        default
            .with_processed_reader(AssetSource::default_reader(p_path.to_owned(), true))
            .with_processed_writer(AssetSource::default_writer(p_path.to_owned(), true))
            .with_processed_watcher(AssetSource::default_watcher(p_path.to_owned(), true, DEBOUNCE))
            .with_processed_watch_warning(AssetSource::default_watch_warning())
    }
}

impl AssetSource {
    /// Returns this [`AssetSourceId`].
    #[inline]
    pub fn id(&self) -> AssetSourceId<'static> {
        self.id.clone()
    }

    /// Return's this source's unprocessed [`ErasedAssetReader`].
    #[inline]
    pub fn reader(&self) -> &dyn ErasedAssetReader {
        &*self.reader
    }

    /// Return's this source's unprocessed [`ErasedAssetWriter`], if it exists.
    #[inline]
    pub fn writer(&self) -> Result<&dyn ErasedAssetWriter, MissingAssetWriter> {
        self.writer
            .as_deref()
            .ok_or_else(|| MissingAssetWriter(self.id.clone_owned()))
    }

    /// Return's this source's processed [`ErasedAssetReader`], if it exists.
    #[inline]
    pub fn processed_reader(&self) -> Result<&dyn ErasedAssetReader, MissingProcessedAssetReader> {
        self.processed_reader
            .as_deref()
            .ok_or_else(|| MissingProcessedAssetReader(self.id.clone_owned()))
    }

    /// Return's this source's processed [`ErasedAssetWriter`], if it exists.
    #[inline]
    pub fn processed_writer(&self) -> Result<&dyn ErasedAssetWriter, MissingProcessedAssetWriter> {
        self.processed_writer
            .as_deref()
            .ok_or_else(|| MissingProcessedAssetWriter(self.id.clone_owned()))
    }

    /// Return's this source's ungated processed [`ErasedAssetWriter`], if it exists.
    #[inline]
    pub(crate) fn ungated_processed_reader(&self) -> Option<&dyn ErasedAssetReader> {
        self.ungated_processed_reader.as_deref()
    }

    /// Return's this source's unprocessed event receiver,
    /// if the source is currently watching for changes.
    #[inline]
    pub fn event_receiver(&self) -> Option<&Receiver<AssetSourceEvent>> {
        self.event_receiver.as_ref()
    }

    /// Return's this source's processed event receiver,
    /// if the source is currently watching for changes.
    #[inline]
    pub fn processed_event_receiver(&self) -> Option<&Receiver<AssetSourceEvent>> {
        self.processed_event_receiver.as_ref()
    }

    /// Returns true if the assets in this source should be processed.
    #[inline]
    pub fn should_process(&self) -> bool {
        self.processed_writer.is_some()
    }

    /// Returns a builder function for this platform's default [`ErasedAssetReader`].
    ///
    /// - `path` is the relative path to the asset root.
    /// - `processed` control whether the data has been processed.
    pub fn default_reader(
        _path: String,
        _processed: bool,
    ) -> impl FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync {
        move || {
            cfg_select! {
                target_arch = "wasm32" => Box::new(crate::io::wasm::HttpWasmAssetReader::new(&_path)),
                target_os = "android" => Box::new(crate::io::android::AndroidAssetReader),
                _ => Box::new(crate::io::file::FileAssetReader::new(&_path)),
            }
        }
    }

    /// Returns a builder function for this platform's default [`ErasedAssetWriter`].
    ///
    /// - `path` is the relative path to the asset root.
    /// - `processed` control whether the data has been processed.
    pub fn default_writer(
        _path: String,
        _processed: bool,
    ) -> impl FnMut() -> Option<Box<dyn ErasedAssetWriter>> + Send + Sync {
        move || {
            cfg_select! {
                target_arch = "wasm32" => None,
                target_os = "android" => None,
                _ => Some(Box::new(crate::io::file::FileAssetWriter::new(&_path, _processed))),
            }
        }
    }

    /// Returns a builder function for this platform's default [`AssetWatcher`].
    ///
    /// - `path` is the relative path to the asset root.
    /// - `processed` control whether the data has been processed.
    pub fn default_watcher(
        _path: String,
        _processed: bool,
        _debounce_wait_time: Duration,
    ) -> impl FnMut(Sender<AssetSourceEvent>) -> Option<Box<dyn AssetWatcher>> + Send + Sync {
        move |_sender: Sender<AssetSourceEvent>| {
            cfg_select! {
                target_arch = "wasm32" => None,
                target_os = "android" => None,
                feature = "file_watcher" => {
                    let path = crate::io::file::base_path().join(_path.clone());
                    if path.exists() {
                        crate::io::watcher::FileWatcher::build(path, _sender, _debounce_wait_time)
                    } else {
                        tracing::warn!("Skip creating file watcher because path {path:?} does not exist.");
                        None
                    }
                }
                _ => None,
            }
        }
    }

    /// Returns a default watch warning message for this platform.
    pub fn default_watch_warning() -> &'static str {
        cfg_select! {
            target_arch = "wasm32" => "Web does not currently support watching assets.",
            target_os = "android" => "Android does not currently support watching assets.",
            feature = "file_watcher" => "Consider adding an \"assets\" directory.",
            _ => "Consider enabling the `file_watcher` feature.",
        }
    }

    pub(crate) fn gate_on_processor(&mut self, processing_state: Arc<ProcessingState>) {
        if let Some(reader) = self.processed_reader.take() {
            self.ungated_processed_reader = Some(reader.clone());

            self.processed_reader = Some(Arc::new(ProcessorGatedReader::new(
                self.id(),
                reader,
                processing_state,
            )));
        }
    }
}

// -----------------------------------------------------------------------------
// Errors

const MISSING_DEFAULT_SOURCE: &str =
    "A default AssetSource is required. Add one to `AssetSourceBuilders`";

// -----------------------------------------------------------------------------
// AssetSources

/// A [`Resource`] that hold (repeatable) functions capable of producing
/// new [`AssetReader`] and [`AssetWriter`] instances for a given asset source.
///
/// [`AssetReader`]: crate::io::AssetReader
/// [`AssetWriter`]: crate::io::AssetWriter
#[derive(Resource, Default)]
pub struct AssetSourceBuilders {
    sources: HashMap<CowArc<'static, str>, AssetSourceBuilder>,
    default: Option<AssetSourceBuilder>,
}

impl AssetSourceBuilders {
    /// Inserts a new builder with the given `id`
    pub fn insert(&mut self, id: impl Into<AssetSourceId<'static>>, source: AssetSourceBuilder) {
        match id.into() {
            AssetSourceId::Default => {
                self.default = Some(source);
            }
            AssetSourceId::Name(name) => {
                self.sources.insert(name, source);
            }
        }
    }

    /// Gets a mutable builder with the given `id`, if it exists.
    pub fn get_mut<'a, 'b>(
        &'a mut self,
        id: impl Into<AssetSourceId<'b>>,
    ) -> Option<&'a mut AssetSourceBuilder> {
        match id.into() {
            AssetSourceId::Default => self.default.as_mut(),
            AssetSourceId::Name(name) => self.sources.get_mut(&name.into_owned()),
        }
    }

    /// Initializes the default [`AssetSourceBuilder`] if it has not already been set.
    pub fn init_default_source(&mut self, path: &str, processed_path: Option<&str>) {
        self.default
            .get_or_insert_with(|| AssetSourceBuilder::platform_default(path, processed_path));
    }

    /// Builds a new [`AssetSources`] collection.
    ///
    /// - If `watch` is true, the unprocessed sources will watch for changes.
    /// - If `watch_processed` is true, the processed sources will watch for changes.
    ///
    /// Note that the default watcher need to enable `file_watcher` cargo feature.
    pub fn build_sources(&mut self, watch: bool, watch_processed: bool) -> AssetSources {
        let mut sources: HashMap<CowArc<'_, str>, AssetSource> = HashMap::new();

        for (key, source) in &mut self.sources {
            let id = AssetSourceId::Name(key.clone_owned());
            let source = source.build(id, watch, watch_processed);
            sources.insert(key.clone_owned(), source);
        }

        let default = self
            .default
            .as_mut()
            .map(|p| p.build(AssetSourceId::Default, watch, watch_processed))
            .expect(MISSING_DEFAULT_SOURCE);

        AssetSources { sources, default }
    }
}

// -----------------------------------------------------------------------------
// AssetSources

/// A collection of [`AssetSource`]s.
pub struct AssetSources {
    sources: HashMap<CowArc<'static, str>, AssetSource>,
    default: AssetSource,
}

impl AssetSources {
    /// Gets the [`AssetSource`] with the given `id`, if it exists.
    pub fn get<'a, 'b>(
        &'a self,
        id: impl Into<AssetSourceId<'b>>,
    ) -> Result<&'a AssetSource, MissingAssetSource> {
        match id.into().into_owned() {
            AssetSourceId::Default => Ok(&self.default),
            AssetSourceId::Name(name) => self
                .sources
                .get(&name)
                .ok_or(MissingAssetSource(AssetSourceId::Name(name))),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &AssetSource> {
        self.sources.values().chain(Some(&self.default))
    }

    /// Mutably iterates all asset sources in the collection (including the default source).
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut AssetSource> {
        self.sources.values_mut().chain(Some(&mut self.default))
    }

    /// Iterates all processed asset sources in the collection (including the default source).
    pub fn iter_processed(&self) -> impl Iterator<Item = &AssetSource> {
        self.iter().filter(|p| p.should_process())
    }

    /// Mutably iterates all processed asset sources in the collection (including the default source).
    pub fn iter_processed_mut(&mut self) -> impl Iterator<Item = &mut AssetSource> {
        self.iter_mut().filter(|p| p.should_process())
    }

    pub fn iter_id(&self) -> impl Iterator<Item = AssetSourceId<'static>> + '_ {
        self.sources
            .keys()
            .map(|k| AssetSourceId::Name(k.clone_owned()))
            .chain(Some(AssetSourceId::Default))
    }

    /// This will cause processed [`AssetReader`] futures (such as `read`) to wait
    /// until the [`AssetProcessor`] has finished processing the requested asset.
    ///
    /// [`AssetReader`]: crate::io::AssetReader
    /// [`AssetProcessor`]: crate::processor::AssetProcessor
    pub(crate) fn gate_on_processor(&mut self, processing_state: Arc<ProcessingState>) {
        for source in self.iter_processed_mut() {
            source.gate_on_processor(processing_state.clone());
        }
    }
}
