use alloc::boxed::Box;
use alloc::string::String;
use core::time::Duration;
use std::borrow::ToOwned;

use atomicow::CowArc;
use thiserror::Error;
use voker_ecs::derive::Resource;
use voker_os::Arc;

use async_channel::{Receiver, Sender};
use voker_utils::hash::HashMap;

use super::AssetSourceEvent;
use crate::ident::AssetSourceId;
use crate::io::{AssetWatcher, ErasedAssetReader, ErasedAssetWriter};

// -----------------------------------------------------------------------------
// AssetSourceBuilder & AssetSource

type ReaderBuilder = Box<dyn FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync>;
type WriterBuilder = Box<dyn FnMut() -> Option<Box<dyn ErasedAssetWriter>> + Send + Sync>;
type WatcherBuilder =
    Box<dyn FnMut(Sender<AssetSourceEvent>) -> Option<Box<dyn AssetWatcher>> + Send + Sync>;

pub struct AssetSourceBuilder {
    pub reader: ReaderBuilder,
    pub writer: Option<WriterBuilder>,
    pub watcher: Option<WatcherBuilder>,
    pub processed_reader: Option<ReaderBuilder>,
    pub processed_writer: Option<WriterBuilder>,
    pub processed_watcher: Option<WatcherBuilder>,
    pub watch_warning: Option<&'static str>,
    pub processed_watch_warning: Option<&'static str>,
}

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
}

// -----------------------------------------------------------------------------
// AssetSourceBuilder Implementation

impl AssetSourceBuilder {
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
            event_receiver: None,
            watcher: None,
            processed_event_receiver: None,
            processed_watcher: None,
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

    #[inline]
    pub fn with_reader(
        mut self,
        reader: impl FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync + 'static,
    ) -> Self {
        self.reader = Box::new(reader);
        self
    }

    #[inline]
    pub fn with_writer(
        mut self,
        writer: impl FnMut() -> Option<Box<dyn ErasedAssetWriter>> + Send + Sync + 'static,
    ) -> Self {
        self.writer = Some(Box::new(writer));
        self
    }

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

    #[inline]
    pub fn with_processed_reader(
        mut self,
        reader: impl FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync + 'static,
    ) -> Self {
        self.processed_reader = Some(Box::new(reader));
        self
    }

    #[inline]
    pub fn with_processed_writer(
        mut self,
        writer: impl FnMut() -> Option<Box<dyn ErasedAssetWriter>> + Send + Sync + 'static,
    ) -> Self {
        self.processed_writer = Some(Box::new(writer));
        self
    }

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

    #[inline]
    pub fn with_watch_warning(mut self, warning: &'static str) -> Self {
        self.watch_warning = Some(warning);
        self
    }

    #[inline]
    pub fn with_processed_watch_warning(mut self, warning: &'static str) -> Self {
        self.processed_watch_warning = Some(warning);
        self
    }

    pub fn platform_default(path: &str, processed_path: Option<&str>) -> Self {
        const WAIT_TIME: Duration = Duration::from_millis(300);

        let default = Self::new(AssetSource::default_reader(path.to_owned(), false))
            .with_writer(AssetSource::default_writer(path.to_owned(), false))
            .with_watcher(AssetSource::default_watcher(
                path.to_owned(),
                false,
                WAIT_TIME,
            ))
            .with_watch_warning(AssetSource::default_watch_warning());

        if let Some(p_path) = processed_path {
            default
                .with_processed_reader(AssetSource::default_reader(p_path.to_owned(), true))
                .with_processed_writer(AssetSource::default_writer(p_path.to_owned(), true))
                .with_processed_watcher(AssetSource::default_watcher(
                    p_path.to_owned(),
                    true,
                    WAIT_TIME,
                ))
                .with_processed_watch_warning(AssetSource::default_watch_warning())
        } else {
            default
        }
    }
}

impl AssetSource {
    #[inline]
    pub fn id(&self) -> AssetSourceId<'static> {
        self.id.clone()
    }

    #[inline]
    pub fn reader(&self) -> &dyn ErasedAssetReader {
        &*self.reader
    }

    #[inline]
    pub fn writer(&self) -> Result<&dyn ErasedAssetWriter, MissingAssetWriterError> {
        self.writer
            .as_deref()
            .ok_or_else(|| MissingAssetWriterError(self.id.clone_owned()))
    }

    #[inline]
    pub fn processed_reader(
        &self,
    ) -> Result<&dyn ErasedAssetReader, MissingProcessedAssetReaderError> {
        self.processed_reader
            .as_deref()
            .ok_or_else(|| MissingProcessedAssetReaderError(self.id.clone_owned()))
    }

    #[inline]
    pub fn processed_writer(
        &self,
    ) -> Result<&dyn ErasedAssetWriter, MissingProcessedAssetWriterError> {
        self.processed_writer
            .as_deref()
            .ok_or_else(|| MissingProcessedAssetWriterError(self.id.clone_owned()))
    }

    #[inline]
    pub fn event_receiver(&self) -> Option<&Receiver<AssetSourceEvent>> {
        self.event_receiver.as_ref()
    }

    #[inline]
    pub fn processed_event_receiver(&self) -> Option<&Receiver<AssetSourceEvent>> {
        self.processed_event_receiver.as_ref()
    }

    #[inline]
    pub fn should_process(&self) -> bool {
        self.processed_writer.is_some()
    }

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

    pub fn default_watcher(
        _path: String,
        _processed: bool,
        _debounce_wait_time: Duration,
    ) -> impl FnMut(Sender<AssetSourceEvent>) -> Option<Box<dyn AssetWatcher>> + Send + Sync {
        move |sender: Sender<AssetSourceEvent>| {
            cfg_select! {
                target_arch = "wasm32" => None,
                target_os = "android" => None,
                feature = "file_watcher" => {
                    let path = crate::io::file::base_path().join(_path.clone());
                    if path.exists() {
                        crate::io::watcher::FileWatcher::new(path, sender, _debounce_wait_time)
                    } else {
                        tracing::warn!("Skip creating file watcher because path {path:?} does not exist.");
                        None
                    }
                }
                _ => None,
            }
        }
    }

    pub fn default_watch_warning() -> &'static str {
        cfg_select! {
            target_arch = "wasm32" => "Web does not currently support watching assets.",
            target_os = "android" => "Android does not currently support watching assets.",
            feature = "file_watcher" => "Consider adding an \"assets\" directory.",
            _ => "Consider enabling the `file_watcher` feature.",
        }
    }
}

// -----------------------------------------------------------------------------
// Errors

/// An error returned when an [`AssetSource`] does not exist for a given id.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("Asset Source '{0}' does not exist")]
pub struct MissingAssetSourceError(AssetSourceId<'static>);

/// An error returned when an [`AssetWriter`](crate::io::AssetWriter) does not exist for a given id.
#[derive(Error, Debug, Clone)]
#[error("Asset Source '{0}' does not have an AssetWriter.")]
pub struct MissingAssetWriterError(AssetSourceId<'static>);

/// An error returned when a processed [`AssetReader`](crate::io::AssetReader) does not exist for a given id.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("Asset Source '{0}' does not have a processed AssetReader.")]
pub struct MissingProcessedAssetReaderError(AssetSourceId<'static>);

/// An error returned when a processed [`AssetWriter`](crate::io::AssetWriter) does not exist for a given id.
#[derive(Error, Debug, Clone)]
#[error("Asset Source '{0}' does not have a processed AssetWriter.")]
pub struct MissingProcessedAssetWriterError(AssetSourceId<'static>);

const MISSING_DEFAULT_SOURCE: &str =
    "A default AssetSource is required. Add one to `AssetSourceBuilders`";

// -----------------------------------------------------------------------------
// AssetSources

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

    pub fn init_default_source(&mut self, path: &str, processed_path: Option<&str>) {
        self.default
            .get_or_insert_with(|| AssetSourceBuilder::platform_default(path, processed_path));
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
    ) -> Result<&'a AssetSource, MissingAssetSourceError> {
        match id.into().into_owned() {
            AssetSourceId::Default => Ok(&self.default),
            AssetSourceId::Name(name) => self
                .sources
                .get(&name)
                .ok_or(MissingAssetSourceError(AssetSourceId::Name(name))),
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
}
