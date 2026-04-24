use async_channel::Sender;
use core::time::Duration;
use alloc::boxed::Box;
use std::path::{Path, PathBuf};

use notify_debouncer_full::notify::RecommendedWatcher;
use notify_debouncer_full::{Debouncer, RecommendedCache};

use super::notifier::{EventNotifier, EventPath};
use super::notifier::{build_debouncer, make_absolute_path};
use crate::io::{AssetSourceEvent, AssetWatcher};

// -----------------------------------------------------------------------------
// FileEventNotifier

struct FileEventNotifier {
    root: PathBuf,
    sender: Sender<AssetSourceEvent>,
    last_event: Option<AssetSourceEvent>,
}

impl EventNotifier for FileEventNotifier {
    fn begin(&mut self) {
        self.last_event = None;
    }

    fn parse(&self, absolute_path: &Path) -> Option<EventPath> {
        let root = &self.root;

        let Ok(relative_path) = absolute_path.strip_prefix(root) else {
            strip_prefix_faild(absolute_path, root);
            return None;
        };

        let is_meta = relative_path.extension().is_some_and(|e| e == "meta");

        let path = if is_meta {
            relative_path.with_extension("")
        } else {
            relative_path.to_path_buf()
        };

        Some(EventPath { path, is_meta })
    }

    fn notify(&mut self, _absolute_paths: &[PathBuf], event: AssetSourceEvent) {
        if self.last_event.as_ref() != Some(&event) {
            self.last_event = Some(event.clone());
            self.sender.send_blocking(event).unwrap();
        }
    }
}

#[cold]
#[inline(never)]
fn strip_prefix_faild<'a>(absolute_path: &'a Path, root: &'a Path) {
    // Should not happen.
    tracing::error!(
        "FileEventNotifier::parse() failed to strip prefix: absolute_path={}, root={}. {}",
        absolute_path.display(),
        root.display(),
        core::panic::Location::caller(),
    );
}

// -----------------------------------------------------------------------------
// FileWatcher

#[cfg(all(feature = "file_watcher", not(target_arch = "wasm32"), not(target_os = "android")))] // For Doc
pub struct FileWatcher {
    _watcher: Debouncer<RecommendedWatcher, RecommendedCache>,
}

impl FileWatcher {
    pub fn new(
        path: PathBuf,
        sender: Sender<AssetSourceEvent>,
        debounce_wait_time: Duration,
    ) -> Option<Box<dyn AssetWatcher>> {
        let root = match make_absolute_path(&path) {
            Ok(r) => r,
            Err(err) => return watch_failed(err),
        };

        let notifier = FileEventNotifier {
            root: root.clone(),
            sender,
            last_event: None,
        };

        match build_debouncer(root, debounce_wait_time, notifier) {
            Ok(watcher) => Some(Box::new(FileWatcher { _watcher: watcher })),
            Err(error) => watch_failed(error),
        }
    }
}

impl AssetWatcher for FileWatcher {}

#[cold]
fn watch_failed(err: impl core::error::Error) -> Option<Box<dyn AssetWatcher>> {
    tracing::error!(
        "Create FileWatcher failed, file assets hot-reload cannot work: {err}."
    );
    None
}

