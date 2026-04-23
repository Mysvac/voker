use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::io::SeekFrom;
use std::path::{Path, PathBuf};

use futures_io::{AsyncRead, AsyncSeek};
use thiserror::Error;
use voker_ecs::error::GameError;

use crate::PathStream;

// -----------------------------------------------------------------------------
// AssetReaderError

/// Errors that occur while loading assets.
#[derive(GameError, Error, Debug)]
#[game_error(severity = "error")]
#[non_exhaustive]
pub enum AssetReaderError {
    #[error("Path not found: {}", _0.display())]
    NotFound(PathBuf),
    #[error("Encountered an I/O error while loading asset: {0}")]
    Io(std::io::Error),
    #[error("Encountered HTTP status {0:?} when loading asset")]
    HttpError(u16),
}

impl Clone for AssetReaderError {
    fn clone(&self) -> Self {
        match self {
            Self::NotFound(arg) => Self::NotFound(arg.clone()),
            Self::Io(arg) => {
                // For IO errors, we only compare types,
                // so `Clone` guarantees equality invariance.
                let kind = arg.kind();
                let error = arg.to_string();
                Self::Io(std::io::Error::new(kind, error))
            }
            Self::HttpError(arg) => Self::HttpError(*arg),
        }
    }
}

impl PartialEq for AssetReaderError {
    /// Equality comparison for `AssetReaderError::Io` is not full (only through `ErrorKind` of inner error)
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::NotFound(path), Self::NotFound(other_path)) => path == other_path,
            (Self::Io(error), Self::Io(other_error)) => error.kind() == other_error.kind(),
            (Self::HttpError(code), Self::HttpError(other_code)) => code == other_code,
            _ => false,
        }
    }
}

impl Eq for AssetReaderError {}

impl From<std::io::Error> for AssetReaderError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

// -----------------------------------------------------------------------------
// Reader

pub trait Reader: AsyncRead + Unpin + Send + Sync {
    fn seekable(&mut self) -> Result<&mut dyn SeekableReader, ReaderNotSeekableError>;

    #[inline]
    fn into_boxed<'a>(self) -> Box<dyn Reader + 'a>
    where
        Self: Sized + 'a,
    {
        Box::new(self)
    }
}

impl Reader for Box<dyn Reader + '_> {
    #[inline(always)]
    fn seekable(&mut self) -> Result<&mut dyn SeekableReader, ReaderNotSeekableError> {
        (**self).seekable()
    }

    #[inline(always)]
    fn into_boxed<'a>(self) -> Box<dyn Reader + 'a>
    where
        Self: Sized + 'a,
    {
        self
    }
}

pub use futures_lite::AsyncReadExt as ReaderExt;

// -----------------------------------------------------------------------------
// SeekableReader

pub trait SeekableReader: Reader + AsyncSeek {}

impl<T: Reader + AsyncSeek> SeekableReader for T {}

#[derive(GameError, Error, Debug, Copy, Clone)]
#[game_error(severity = "warning")]
#[error("The `Reader` returned by `AssetReader` does not support `AsyncSeek` behavior.")]
pub struct ReaderNotSeekableError;

// -----------------------------------------------------------------------------
// AssetReader

pub trait AssetReader: Sized + Sync + Send + 'static {
    /// Returns a future to load the full file data at the provided path.
    fn read<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = Result<impl Reader + 'a, AssetReaderError>> + Send;

    /// Returns a future to load the full file data at the provided path.
    fn read_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = Result<impl Reader + 'a, AssetReaderError>> + Send;

    /// Returns an iterator of directory entry names at the provided path.
    fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = Result<Box<PathStream>, AssetReaderError>> + Send;

    /// Returns true if the provided path points to a directory.
    fn is_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = Result<bool, AssetReaderError>> + Send;

    fn read_bytes<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = Result<Vec<u8>, AssetReaderError>> + Send {
        async {
            let mut data_reader = self.read(path).await?;
            let mut data_bytes = Vec::new();
            data_reader.read_to_end(&mut data_bytes).await?; // AsyncReadExt
            Ok(data_bytes)
        }
    }

    fn read_meta_bytes<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = Result<Vec<u8>, AssetReaderError>> + Send {
        async {
            let mut meta_reader = self.read_meta(path).await?;
            let mut meta_bytes = Vec::new();
            meta_reader.read_to_end(&mut meta_bytes).await?; // AsyncReadExt
            Ok(meta_bytes)
        }
    }
}

// -----------------------------------------------------------------------------
// ErasedAssetReader

pub type BoxedAssetReaderFuture<'a, T> = crate::BoxedFuture<'a, Result<T, AssetReaderError>>;

pub trait ErasedAssetReader: Send + Sync + 'static {
    /// Returns a future to load the full file data at the provided path.
    fn read<'a>(&'a self, path: &'a Path) -> BoxedAssetReaderFuture<'a, Box<dyn Reader + 'a>>;

    /// Returns a future to load the full file data at the provided path.
    fn read_meta<'a>(&'a self, path: &'a Path) -> BoxedAssetReaderFuture<'a, Box<dyn Reader + 'a>>;

    /// Returns an iterator of directory entry names at the provided path.
    fn read_directory<'a>(&'a self, path: &'a Path) -> BoxedAssetReaderFuture<'a, Box<PathStream>>;

    /// Returns true if the provided path points to a directory.
    fn is_directory<'a>(&'a self, path: &'a Path) -> BoxedAssetReaderFuture<'a, bool>;

    fn read_bytes<'a>(&'a self, path: &'a Path) -> BoxedAssetReaderFuture<'a, Vec<u8>>;

    fn read_meta_bytes<'a>(&'a self, path: &'a Path) -> BoxedAssetReaderFuture<'a, Vec<u8>>;
}

impl<T: AssetReader> ErasedAssetReader for T {
    fn read<'a>(&'a self, path: &'a Path) -> BoxedAssetReaderFuture<'a, Box<dyn Reader + 'a>> {
        Box::pin(async move { Ok(<T as AssetReader>::read(self, path).await?.into_boxed()) })
    }

    fn read_meta<'a>(&'a self, path: &'a Path) -> BoxedAssetReaderFuture<'a, Box<dyn Reader + 'a>> {
        Box::pin(async move { Ok(<T as AssetReader>::read_meta(self, path).await?.into_boxed()) })
    }

    fn read_directory<'a>(&'a self, path: &'a Path) -> BoxedAssetReaderFuture<'a, Box<PathStream>> {
        Box::pin(<T as AssetReader>::read_directory(self, path))
    }

    fn is_directory<'a>(&'a self, path: &'a Path) -> BoxedAssetReaderFuture<'a, bool> {
        Box::pin(<T as AssetReader>::is_directory(self, path))
    }

    fn read_bytes<'a>(&'a self, path: &'a Path) -> BoxedAssetReaderFuture<'a, Vec<u8>> {
        Box::pin(<T as AssetReader>::read_bytes(self, path))
    }

    fn read_meta_bytes<'a>(&'a self, path: &'a Path) -> BoxedAssetReaderFuture<'a, Vec<u8>> {
        Box::pin(<T as AssetReader>::read_meta_bytes(self, path))
    }
}

// -----------------------------------------------------------------------------
// VecReader

/// An [`AsyncRead`] implementation capable of reading a [`Vec<u8>`].
pub struct VecReader {
    /// The bytes being read. This is the full original list of bytes.
    pub bytes: Vec<u8>,
    bytes_read: usize,
}

impl VecReader {
    /// Create a new [`VecReader`] for `bytes`.
    #[inline(always)]
    pub const fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            bytes_read: 0,
        }
    }
}

impl AsyncRead for VecReader {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<futures_io::Result<usize>> {
        use crate::utils::slice_read;
        let this = self.get_mut();
        Poll::Ready(Ok(slice_read(&this.bytes, &mut this.bytes_read, buf)))
    }
}

impl AsyncSeek for VecReader {
    #[inline]
    fn poll_seek(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<std::io::Result<u64>> {
        use crate::utils::slice_seek;
        // Get the mut borrow to avoid trying to borrow the pin itself multiple times.
        let this = self.get_mut();
        Poll::Ready(slice_seek(&this.bytes, &mut this.bytes_read, pos))
    }
}

impl Reader for VecReader {
    #[inline(always)]
    fn seekable(&mut self) -> Result<&mut dyn SeekableReader, ReaderNotSeekableError> {
        Ok(self)
    }
}

// -----------------------------------------------------------------------------
// SliceReader

/// An [`AsyncRead`] implementation capable of reading a [`&[u8]`].
pub struct SliceReader<'a> {
    bytes: &'a [u8],
    bytes_read: usize,
}

impl<'a> SliceReader<'a> {
    /// Create a new [`SliceReader`] for `bytes`.
    #[inline(always)]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            bytes_read: 0,
        }
    }
}

impl AsyncRead for SliceReader<'_> {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        use crate::utils::slice_read;
        let this = self.get_mut();
        Poll::Ready(Ok(slice_read(this.bytes, &mut this.bytes_read, buf)))
    }
}

impl AsyncSeek for SliceReader<'_> {
    #[inline]
    fn poll_seek(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<std::io::Result<u64>> {
        use crate::utils::slice_seek;
        let this = self.get_mut();
        Poll::Ready(slice_seek(this.bytes, &mut this.bytes_read, pos))
    }
}

impl Reader for SliceReader<'_> {
    #[inline(always)]
    fn seekable(&mut self) -> Result<&mut dyn SeekableReader, ReaderNotSeekableError> {
        Ok(self)
    }
}
