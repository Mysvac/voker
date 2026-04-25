use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::pin::Pin;
use core::task::Poll;
use std::path::Path;

use async_lock::RwLockReadGuardArc;
use futures_io::AsyncRead;

use super::ErasedAssetReader;
use super::{AssetReader, AssetReaderError};
use super::{Reader, ReaderNotSeekableError, SeekableReader};
use crate::PathStream;
use crate::ident::AssetSourceId;
use crate::path::AssetPath;
use crate::processor::{ProcessStatus, ProcessingState};

// -----------------------------------------------------------------------------
// ProcessorGatedReader

/// An [`AssetReader`] that will prevent asset (and asset metadata) read futures from
/// returning for a given path until that path has been processed by [`AssetProcessor`].
///
/// [`AssetProcessor`]: crate::processor::AssetProcessor
pub(crate) struct ProcessorGatedReader {
    reader: Arc<dyn ErasedAssetReader>,
    source: AssetSourceId<'static>,
    processing_state: Arc<ProcessingState>,
}

impl ProcessorGatedReader {
    /// Creates a new [`ProcessorGatedReader`].
    pub(crate) fn new(
        source: AssetSourceId<'static>,
        reader: Arc<dyn ErasedAssetReader>,
        processing_state: Arc<ProcessingState>,
    ) -> Self {
        Self {
            source,
            reader,
            processing_state,
        }
    }
}

impl AssetReader for ProcessorGatedReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let asset_path = AssetPath::from(path.to_path_buf()).with_source(self.source.clone());
        tracing::trace!("Waiting for processing to finish before reading {asset_path}");

        let process_result = self.processing_state.wait_until_processed(asset_path.clone()).await;
        match process_result {
            ProcessStatus::Processed => {}
            ProcessStatus::Failed | ProcessStatus::NonExistent => {
                return Err(AssetReaderError::NotFound(path.to_owned()));
            }
        }

        tracing::trace!("Processing finished with {asset_path}, reading {process_result:?}",);
        let lock = self.processing_state.get_transaction_lock(&asset_path).await?;
        let asset_reader = self.reader.read(path).await?;
        let reader = TransactionLockedReader::new(asset_reader, lock);
        Ok(reader)
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let asset_path = AssetPath::from(path.to_path_buf()).with_source(self.source.clone());
        tracing::trace!("Waiting for processing to finish before reading meta for {asset_path}",);

        let process_result = self.processing_state.wait_until_processed(asset_path.clone()).await;
        match process_result {
            ProcessStatus::Processed => {}
            ProcessStatus::Failed | ProcessStatus::NonExistent => {
                return Err(AssetReaderError::NotFound(path.to_owned()));
            }
        }

        tracing::trace!(
            "Processing finished with {process_result:?}, reading meta for {asset_path}",
        );
        let lock = self.processing_state.get_transaction_lock(&asset_path).await?;
        let meta_reader = self.reader.read_meta(path).await?;
        let reader = TransactionLockedReader::new(meta_reader, lock);
        Ok(reader)
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        tracing::trace!("Waiting for processing to finish before reading directory {path:?}");
        self.processing_state.wait_until_finished().await;

        tracing::trace!("Processing finished, reading directory {path:?}");
        self.reader.read_directory(path).await
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        tracing::trace!("Waiting for processing to finish before reading directory {path:?}");
        self.processing_state.wait_until_finished().await;

        tracing::trace!("Processing finished, getting directory status {path:?}");
        self.reader.is_directory(path).await
    }
}

// -----------------------------------------------------------------------------
// TransactionLockedReader

/// An [`AsyncRead`] impl that will hold its asset's transaction lock
/// until [`TransactionLockedReader`] is dropped.
struct TransactionLockedReader<'a> {
    reader: Box<dyn Reader + 'a>,
    _file_transaction_lock: RwLockReadGuardArc<()>,
}

impl<'a> TransactionLockedReader<'a> {
    fn new(reader: Box<dyn Reader + 'a>, file_transaction_lock: RwLockReadGuardArc<()>) -> Self {
        Self {
            reader,
            _file_transaction_lock: file_transaction_lock,
        }
    }
}

impl AsyncRead for TransactionLockedReader<'_> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<futures_io::Result<usize>> {
        Pin::new(&mut self.reader).poll_read(cx, buf)
    }
}

impl Reader for TransactionLockedReader<'_> {
    #[inline]
    fn seekable(&mut self) -> Result<&mut dyn SeekableReader, ReaderNotSeekableError> {
        self.reader.seekable()
    }

    #[inline]
    fn read_all_bytes<'a>(&'a mut self, buf: &'a mut Vec<u8>) -> super::future::ReadAllFuture<'a> {
        self.reader.read_all_bytes(buf)
    }
}
