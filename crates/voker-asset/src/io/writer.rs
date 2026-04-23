use alloc::boxed::Box;
use alloc::string::ToString;
use std::path::{Path, PathBuf};

use futures_io::AsyncWrite;
use futures_lite::AsyncWriteExt;
use thiserror::Error;
use voker_ecs::error::GameError;

// -----------------------------------------------------------------------------
// AssetWriterError

#[derive(GameError, Error, Debug)]
#[game_error(severity = "error")]
#[non_exhaustive]
pub enum AssetWriterError {
    #[error("The Path is invalid: {}", _0.display())]
    InvalidPath(PathBuf),
    #[error("Encountered an I/O error while loading asset: {0}")]
    Io(std::io::Error),
}

impl Clone for AssetWriterError {
    fn clone(&self) -> Self {
        match self {
            Self::InvalidPath(arg) => Self::InvalidPath(arg.clone()),
            Self::Io(arg) => {
                // For IO errors, we only compare types,
                // so `Clone` guarantees equality invariance.
                let kind = arg.kind();
                let error = arg.to_string();
                Self::Io(std::io::Error::new(kind, error))
            }
        }
    }
}

impl PartialEq for AssetWriterError {
    /// Equality comparison for `AssetReaderError::Io` is not full (only through `ErrorKind` of inner error)
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InvalidPath(p1), Self::InvalidPath(p2)) => *p1 == *p2,
            (Self::Io(e1), Self::Io(e2)) => e1.kind() == e2.kind(),
            _ => false,
        }
    }
}

impl Eq for AssetWriterError {}

impl From<std::io::Error> for AssetWriterError {
    #[inline]
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

// -----------------------------------------------------------------------------
// Writer

pub trait Writer: AsyncWrite + Unpin + Send + Sync {
    #[inline]
    fn into_boxed<'a>(self) -> Box<dyn Writer + 'a>
    where
        Self: Sized + 'a,
    {
        Box::new(self)
    }
}

impl Writer for Box<dyn Writer + '_> {
    #[inline(always)]
    fn into_boxed<'a>(self) -> Box<dyn Writer + 'a>
    where
        Self: Sized + 'a,
    {
        self
    }
}

pub use futures_lite::AsyncWriteExt as WriterExt;

// -----------------------------------------------------------------------------
// AssetWriter

pub trait AssetWriter: Send + Sync + 'static {
    /// Writes the full asset bytes at the provided path.
    fn write<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = Result<impl Writer + 'a, AssetWriterError>> + Send;

    /// Writes the full asset meta bytes at the provided path.
    ///
    /// This _should not_ include storage specific extensions like `.meta`.
    fn write_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = Result<impl Writer + 'a, AssetWriterError>> + Send;

    /// Removes the asset stored at the given path.
    fn remove<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = Result<(), AssetWriterError>> + Send;

    /// Removes the asset meta stored at the given path.
    ///
    /// This _should not_ include storage specific extensions like `.meta`.
    fn remove_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = Result<(), AssetWriterError>> + Send;

    /// Renames the asset at `old_path` to `new_path`
    fn rename<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> impl Future<Output = Result<(), AssetWriterError>> + Send;

    /// Renames the asset meta for the asset at `old_path` to `new_path`.
    ///
    /// This _should not_ include storage specific extensions like `.meta`.
    fn rename_meta<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> impl Future<Output = Result<(), AssetWriterError>> + Send;

    /// Creates a directory at the given path, including all parent directories if they do not
    /// already exist.
    fn create_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = Result<(), AssetWriterError>> + Send;

    /// Removes the directory at the given path, including all assets _and_ directories in that directory.
    fn remove_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = Result<(), AssetWriterError>> + Send;

    /// Removes the directory at the given path, but only if it is completely empty.
    ///
    /// This will return an error if the directory is not empty.
    fn remove_empty_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = Result<(), AssetWriterError>> + Send;

    /// Removes all assets (and directories) in this directory, resulting in an empty directory.
    fn remove_assets_in_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = Result<(), AssetWriterError>> + Send;

    /// Writes the asset `bytes` to the given `path`.
    fn write_bytes<'a>(
        &'a self,
        path: &'a Path,
        bytes: &'a [u8],
    ) -> impl Future<Output = Result<(), AssetWriterError>> + Send {
        async {
            let mut writer = self.write(path).await?;
            writer.write_all(bytes).await?; // AsyncReadExt
            writer.flush().await?;
            Ok(())
        }
    }
    /// Writes the asset meta `bytes` to the given `path`.
    fn write_meta_bytes<'a>(
        &'a self,
        path: &'a Path,
        bytes: &'a [u8],
    ) -> impl Future<Output = Result<(), AssetWriterError>> + Send {
        async {
            let mut meta_writer = self.write_meta(path).await?;
            meta_writer.write_all(bytes).await?; // AsyncReadExt
            meta_writer.flush().await?;
            Ok(())
        }
    }
}

// -----------------------------------------------------------------------------
// ErasedAssetWriter

type BoxedAssetWriterFuture<'a, T> = crate::BoxedFuture<'a, Result<T, AssetWriterError>>;

pub trait ErasedAssetWriter: Send + Sync + 'static {
    /// Writes the full asset bytes at the provided path.
    fn write<'a>(&'a self, path: &'a Path) -> BoxedAssetWriterFuture<'a, Box<dyn Writer + 'a>>;

    /// Writes the full asset meta bytes at the provided path.
    ///
    /// This _should not_ include storage specific extensions like `.meta`.
    fn write_meta<'a>(&'a self, path: &'a Path)
    -> BoxedAssetWriterFuture<'a, Box<dyn Writer + 'a>>;

    /// Removes the asset stored at the given path.
    fn remove<'a>(&'a self, path: &'a Path) -> BoxedAssetWriterFuture<'a, ()>;

    /// Removes the asset meta stored at the given path.
    ///
    /// This _should not_ include storage specific extensions like `.meta`.
    fn remove_meta<'a>(&'a self, path: &'a Path) -> BoxedAssetWriterFuture<'a, ()>;

    /// Renames the asset at `old_path` to `new_path`
    fn rename<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> BoxedAssetWriterFuture<'a, ()>;

    /// Renames the asset meta for the asset at `old_path` to `new_path`.
    ///
    /// This _should not_ include storage specific extensions like `.meta`.
    fn rename_meta<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> BoxedAssetWriterFuture<'a, ()>;

    /// Creates a directory at the given path, including all parent directories if they do not
    /// already exist.
    fn create_directory<'a>(&'a self, path: &'a Path) -> BoxedAssetWriterFuture<'a, ()>;

    /// Removes the directory at the given path, including all assets _and_ directories in that directory.
    fn remove_directory<'a>(&'a self, path: &'a Path) -> BoxedAssetWriterFuture<'a, ()>;

    /// Removes the directory at the given path, but only if it is completely empty.
    ///
    /// This will return an error if the directory is not empty.
    fn remove_empty_directory<'a>(&'a self, path: &'a Path) -> BoxedAssetWriterFuture<'a, ()>;

    /// Removes all assets (and directories) in this directory, resulting in an empty directory.
    fn remove_assets_in_directory<'a>(&'a self, path: &'a Path) -> BoxedAssetWriterFuture<'a, ()>;

    /// Writes the asset `bytes` to the given `path`.
    fn write_bytes<'a>(&'a self, path: &'a Path, bytes: &'a [u8])
    -> BoxedAssetWriterFuture<'a, ()>;

    /// Writes the asset meta `bytes` to the given `path`.
    fn write_meta_bytes<'a>(
        &'a self,
        path: &'a Path,
        bytes: &'a [u8],
    ) -> BoxedAssetWriterFuture<'a, ()>;
}

impl<T: AssetWriter> ErasedAssetWriter for T {
    fn write<'a>(&'a self, path: &'a Path) -> BoxedAssetWriterFuture<'a, Box<dyn Writer + 'a>> {
        Box::pin(async move { Ok(<T as AssetWriter>::write(self, path).await?.into_boxed()) })
    }

    fn write_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxedAssetWriterFuture<'a, Box<dyn Writer + 'a>> {
        Box::pin(async move { Ok(<T as AssetWriter>::write_meta(self, path).await?.into_boxed()) })
    }

    fn remove<'a>(&'a self, path: &'a Path) -> BoxedAssetWriterFuture<'a, ()> {
        Box::pin(<T as AssetWriter>::remove(self, path))
    }

    fn remove_meta<'a>(&'a self, path: &'a Path) -> BoxedAssetWriterFuture<'a, ()> {
        Box::pin(<T as AssetWriter>::remove_meta(self, path))
    }

    fn rename<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> BoxedAssetWriterFuture<'a, ()> {
        Box::pin(<T as AssetWriter>::rename(self, old_path, new_path))
    }

    fn rename_meta<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> BoxedAssetWriterFuture<'a, ()> {
        Box::pin(<T as AssetWriter>::rename_meta(self, old_path, new_path))
    }

    fn create_directory<'a>(&'a self, path: &'a Path) -> BoxedAssetWriterFuture<'a, ()> {
        Box::pin(<T as AssetWriter>::create_directory(self, path))
    }

    fn remove_directory<'a>(&'a self, path: &'a Path) -> BoxedAssetWriterFuture<'a, ()> {
        Box::pin(<T as AssetWriter>::remove_directory(self, path))
    }

    fn remove_empty_directory<'a>(&'a self, path: &'a Path) -> BoxedAssetWriterFuture<'a, ()> {
        Box::pin(<T as AssetWriter>::remove_empty_directory(self, path))
    }

    fn remove_assets_in_directory<'a>(&'a self, path: &'a Path) -> BoxedAssetWriterFuture<'a, ()> {
        Box::pin(<T as AssetWriter>::remove_assets_in_directory(self, path))
    }

    fn write_bytes<'a>(
        &'a self,
        path: &'a Path,
        bytes: &'a [u8],
    ) -> BoxedAssetWriterFuture<'a, ()> {
        Box::pin(<T as AssetWriter>::write_bytes(self, path, bytes))
    }

    fn write_meta_bytes<'a>(
        &'a self,
        path: &'a Path,
        bytes: &'a [u8],
    ) -> BoxedAssetWriterFuture<'a, ()> {
        Box::pin(<T as AssetWriter>::write_meta_bytes(self, path, bytes))
    }
}
