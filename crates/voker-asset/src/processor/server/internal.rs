use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use async_lock::RwLockReadGuardArc;
use voker_os::sync;
use voker_utils::hash::HashMap;
use voker_utils::hash::map::Entry;

use super::ProcessorState;
use crate::io::{AssetReaderError, AssetSources};
use crate::path::AssetPath;
use crate::processor::{ErasedAssetProcessor, ProcessStatus, ProcessorAssetInfos};
use crate::processor::{MissingProcessor, TransactionLog, TransactionLogFactory};

// -----------------------------------------------------------------------------
// AssetProcessorData

pub(crate) struct AssetProcessorData {
    pub(crate) sources: Arc<AssetSources>,
    pub(crate) state: Arc<ProcessingState>,
    pub(crate) processors: sync::RwLock<Processors>,
    /// The factory that creates the transaction log.
    ///
    /// Note: we use a regular Mutex instead of an async mutex since we expect users to only set
    /// this once, and before the asset processor starts - there is no reason to await (and it
    /// avoids needing to use [`block_on`](bevy_tasks::block_on) to set the factory).
    pub(crate) log_factory: sync::Mutex<Option<Box<dyn TransactionLogFactory>>>,
    pub(crate) log: async_lock::RwLock<Option<Box<dyn TransactionLog>>>,
}

// -----------------------------------------------------------------------------
// ProcessingState

pub(crate) struct ProcessingState {
    /// The overall state of processing.
    state: async_lock::RwLock<ProcessorState>,
    /// The channel to broadcast when the processor has completed initialization.
    pub(crate) initialized_sender: async_broadcast::Sender<()>,
    pub(crate) initialized_receiver: async_broadcast::Receiver<()>,
    /// The channel to broadcast when the processor has completed processing.
    pub(crate) finished_sender: async_broadcast::Sender<()>,
    pub(crate) finished_receiver: async_broadcast::Receiver<()>,
    /// The current state of the assets.
    pub(crate) asset_infos: async_lock::RwLock<ProcessorAssetInfos>,
}

// -----------------------------------------------------------------------------
// ProcessorState

// -----------------------------------------------------------------------------
// Processors

#[derive(Default)]
pub(crate) struct Processors {
    /// Maps the type path of the processor to its instance.
    type_path_to_processor: HashMap<&'static str, Arc<dyn ErasedAssetProcessor>>,
    /// Maps the short type path of the processor to its instance.
    type_name_to_processor: HashMap<&'static str, TypeNameProcessorEntry>,
    /// Maps the file extension of an asset to the type path of the processor we should use to
    /// process it by default.
    file_extension_to_default_processor: HashMap<Box<str>, &'static str>,
}

enum TypeNameProcessorEntry {
    Unique {
        type_path: &'static str,
        processor: Arc<dyn ErasedAssetProcessor>,
    },
    Ambiguous(Vec<&'static str>),
}

// -----------------------------------------------------------------------------
// Processors Implementation

impl Processors {
    /// Registers a processor, making it available by both type path and type name.
    pub(crate) fn push(&mut self, processor: Arc<dyn ErasedAssetProcessor>) {
        let type_path = processor.type_path();
        let type_name = processor.type_name();

        self.type_path_to_processor.insert(type_path, processor.clone());

        match self.type_name_to_processor.entry(type_name) {
            Entry::Occupied(mut entry) => match entry.get_mut() {
                TypeNameProcessorEntry::Unique {
                    type_path: existing,
                    ..
                } => {
                    let existing = *existing;
                    *entry.get_mut() =
                        TypeNameProcessorEntry::Ambiguous(alloc::vec![existing, type_path]);
                }
                TypeNameProcessorEntry::Ambiguous(paths) => {
                    if !paths.contains(&type_path) {
                        paths.push(type_path);
                    }
                }
            },
            Entry::Vacant(entry) => {
                entry.insert(TypeNameProcessorEntry::Unique {
                    type_path,
                    processor,
                });
            }
        }
    }

    /// Gets a processor by its fully-qualified type path.
    pub(crate) fn get(&self, type_path: &str) -> Option<Arc<dyn ErasedAssetProcessor>> {
        self.type_path_to_processor.get(type_path).cloned()
    }

    /// Gets a processor by its short type name.
    ///
    /// Returns `Err` if the name is ambiguous between multiple processors.
    pub(crate) fn get_by_name(
        &self,
        type_name: &str,
    ) -> Result<Option<Arc<dyn ErasedAssetProcessor>>, MissingProcessor> {
        match self.type_name_to_processor.get(type_name) {
            None => Ok(None),
            Some(TypeNameProcessorEntry::Unique { processor, .. }) => Ok(Some(processor.clone())),
            Some(TypeNameProcessorEntry::Ambiguous(paths)) => Err(MissingProcessor::Ambiguous {
                name: type_name.into(),
                ambiguous: paths.clone(),
            }),
        }
    }

    /// Sets the default processor for a given file extension.
    pub(crate) fn set_default_processor(
        &mut self,
        extension: impl Into<Box<str>>,
        type_path: &'static str,
    ) {
        self.file_extension_to_default_processor
            .insert(extension.into(), type_path);
    }

    /// Returns the default processor for the given file extension, if one is registered.
    pub(crate) fn get_default_processor_by_extension(
        &self,
        extension: &str,
    ) -> Option<Arc<dyn ErasedAssetProcessor>> {
        let type_path = self.file_extension_to_default_processor.get(extension)?;
        self.type_path_to_processor.get(type_path).cloned()
    }
}

// -----------------------------------------------------------------------------
// ProcessingState Implementation

impl ProcessingState {
    pub(crate) fn new() -> Self {
        let (mut initialized_sender, initialized_receiver) = async_broadcast::broadcast(1);
        let (mut finished_sender, finished_receiver) = async_broadcast::broadcast(1);
        // allow overflow on these "one slot" channels to allow receivers to retrieve the "latest" state,
        // and to allow senders to not block if there was older state present.
        initialized_sender.set_overflow(true);
        finished_sender.set_overflow(true);

        Self {
            state: async_lock::RwLock::new(ProcessorState::Initializing),
            initialized_sender,
            initialized_receiver,
            finished_sender,
            finished_receiver,
            asset_infos: Default::default(),
        }
    }

    /// Sets the overall state of processing and broadcasts appropriate events.
    pub(crate) async fn set_state(&self, state: ProcessorState) {
        let mut state_guard = self.state.write().await;
        let last_state = *state_guard;
        *state_guard = state;
        ::core::mem::drop(state_guard);
        if last_state != ProcessorState::Finished && state == ProcessorState::Finished {
            self.finished_sender.broadcast(()).await.unwrap();
        } else if last_state != ProcessorState::Processing && state == ProcessorState::Processing {
            self.initialized_sender.broadcast(()).await.unwrap();
        }
    }

    /// Retrieves the current [`ProcessorState`]
    pub(crate) async fn get_state(&self) -> ProcessorState {
        *self.state.read().await
    }

    /// Gets a "transaction lock" that can be used to ensure no writes
    /// to asset or asset meta occur while it is held.
    pub(crate) async fn get_transaction_lock(
        &self,
        path: &AssetPath<'static>,
    ) -> Result<RwLockReadGuardArc<()>, AssetReaderError> {
        let lock = {
            let infos = self.asset_infos.read().await;
            let info = infos
                .get(path)
                .ok_or_else(|| AssetReaderError::NotFound(path.path().to_owned()))?;
            // Clone out the transaction lock first and then lock after we've dropped the
            // asset_infos. Otherwise, trying to lock a single path can block all other paths to
            // (leading to deadlocks).
            info.file_transaction_lock.clone()
        };
        Ok(lock.read_arc().await)
    }

    /// Returns a future that will not finish until the processor has been initialized.
    pub(crate) async fn wait_until_initialized(&self) {
        let receiver = {
            let state = self.state.read().await;
            match *state {
                ProcessorState::Initializing => {
                    // This receiver must be created prior to losing
                    // the read lock to ensure this is transactional
                    Some(self.initialized_receiver.clone())
                }
                _ => None,
            }
        };

        if let Some(mut receiver) = receiver {
            receiver.recv().await.unwrap();
        }
    }

    /// Returns a future that will not finish until processing has finished.
    pub(crate) async fn wait_until_finished(&self) {
        let receiver = {
            let state = self.state.read().await;
            match *state {
                ProcessorState::Initializing | ProcessorState::Processing => {
                    // This receiver must be created prior to losing
                    // the read lock to ensure this is transactional
                    Some(self.finished_receiver.clone())
                }
                ProcessorState::Finished => None,
            }
        };

        if let Some(mut receiver) = receiver {
            receiver.recv().await.unwrap();
        }
    }

    /// Returns a future that will not finish until the path has been processed.
    pub(crate) async fn wait_until_processed(&self, path: AssetPath<'static>) -> ProcessStatus {
        self.wait_until_initialized().await;

        let mut receiver = {
            let infos = self.asset_infos.write().await;
            let info = infos.get(&path);
            match info {
                Some(info) => match info.status {
                    Some(result) => return result,
                    // This receiver must be created prior to losing
                    // the read lock to ensure this is transactional
                    None => info.status_receiver.clone(),
                },
                None => return ProcessStatus::NonExistent,
            }
        };

        receiver.recv().await.unwrap()
    }
}
