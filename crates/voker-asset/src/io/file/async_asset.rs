

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::pin::Pin;
use core::task::Poll;
use std::path::{Path, PathBuf};

use async_fs::File;
use futures_io::{AsyncRead, AsyncWrite};
use futures_lite::StreamExt;

use crate::io::{AssetReader, AssetReaderError, Reader};
use crate::io::{AssetWriter, AssetWriterError, Writer};
use crate::io::{ReaderNotSeekableError, SeekableReader};
use crate::io::future::{ReadAllFuture, WriteAllFuture};
use crate::utils::append_meta_extension;
use super::{FileAssetReader, FileAssetWriter};

// -----------------------------------------------------------------------------
// Open File Limiter

#[cfg(windows)]
use core::marker::PhantomData;

#[cfg(not(windows))]
use async_lock::{Semaphore, SemaphoreGuard};

// Set to OS default limit / 2
// macos & ios: 256
// linux & android: 1024
// windows: none
#[cfg(any(target_os = "macos", target_os = "ios"))]
static OPEN_FILE_LIMITER: Semaphore = Semaphore::new(128);

#[cfg(not(any(target_os = "macos", target_os = "ios", windows)))]
static OPEN_FILE_LIMITER: Semaphore = Semaphore::new(512);

#[cfg(not(target_os = "windows"))]
async fn maybe_get_semaphore<'a>() -> Option<SemaphoreGuard<'a>> {
    use core::time::Duration;
    use futures_util::{future, pin_mut};
    use async_io::Timer;

    let guard_future = OPEN_FILE_LIMITER.acquire();
    let timeout_future = Timer::after(Duration::from_millis(500));
    pin_mut!(guard_future);
    pin_mut!(timeout_future);

    match future::select(guard_future, timeout_future).await {
        future::Either::Left((guard, _)) => Some(guard),
        future::Either::Right((_, _)) => None,
    }
}

// -----------------------------------------------------------------------------
// FileReader

impl Reader for File {
    #[inline(always)]
    fn seekable(&mut self) -> Result<&mut dyn SeekableReader, ReaderNotSeekableError> {
        Ok(self)
    }

    #[inline(always)]
    fn read_all_bytes<'a>(&'a mut self, buf: &'a mut Vec<u8>) -> ReadAllFuture<'a> {
        ReadAllFuture::async_read::<File>(self, buf)
    }
}

struct FileReader<'a> {
    file: File,
    #[cfg(windows)]
    _guard: PhantomData<&'a ()>,
    #[cfg(not(windows))]
    _guard: Option<SemaphoreGuard<'a>>,
}

impl AsyncRead for FileReader<'_> {
    #[inline(always)]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.file).poll_read(cx, buf)
    }
}

impl Reader for FileReader<'_> {
    #[inline(always)]
    fn seekable(&mut self) -> Result<&mut dyn SeekableReader, ReaderNotSeekableError> {
        Ok(&mut self.file)
    }

    #[inline(always)]
    fn read_all_bytes<'a>(&'a mut self, buf: &'a mut Vec<u8>) -> ReadAllFuture<'a> {
        ReadAllFuture::async_read::<File>(&mut self.file, buf)
    }
}

// -----------------------------------------------------------------------------
// AssetReader

#[cold]
fn map_reader_error(e: std::io::Error, path: PathBuf) -> AssetReaderError {
    use std::io::ErrorKind;
    match e.kind() {
        ErrorKind::NotFound => AssetReaderError::NotFound(path),
        _ => AssetReaderError::Io(e)
    }
}

impl AssetReader for FileAssetReader {
    async fn read<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<impl Reader + 'a, AssetReaderError> {
        #[cfg(windows)]
        let _guard = PhantomData;
        #[cfg(not(windows))]
        let _guard = maybe_get_semaphore().await;

        let full_path = self.root_path.join(path);
        match File::open(&full_path).await {
            Ok(file) => Ok(FileReader{ file, _guard }),
            Err(e) => Err(map_reader_error(e, full_path)),
        }
    }

    async fn read_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<impl Reader + 'a, AssetReaderError> {
        #[cfg(windows)]
        let _guard = PhantomData;
        #[cfg(not(windows))]
        let _guard = maybe_get_semaphore().await;

        let meta_path = append_meta_extension(path);
        let full_path = self.root_path.join(meta_path);
        match File::open(&full_path).await {
            Ok(file) => Ok(FileReader{ file, _guard }),
            Err(e) => Err(map_reader_error(e, full_path)),
        }
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<crate::PathStream>, AssetReaderError> {
        let full_path = self.root_path.join(path);

        let read_dir = match async_fs::read_dir(&full_path).await {
            Ok(read_dir) => read_dir,
            Err(e) => return Err(map_reader_error(e, full_path)),
        };

        let root_path = self.root_path.clone();

        let mapped_stream = read_dir.filter_map(move |f| {
            let dir_entry = f.ok()?;
            let path = dir_entry.path();
            // filter out meta files as they are not considered assets
            if let Some(ext) = path.extension().and_then(|e| e.to_str())
                && ext.eq_ignore_ascii_case("meta")
            {
                return None;
            }

            // filter out hidden files. they are not listed by default but are directly targetable
            if let Some(file_name) = path.file_name()
                && file_name.as_encoded_bytes().first() == Some(&b'.')
            {
                return None;
            }

            let relative_path = path.strip_prefix(&root_path).unwrap();
            Some(relative_path.to_path_buf())
        });

        Ok(Box::new(mapped_stream))
    }

    async fn is_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<bool, AssetReaderError> {
        let full_path = self.root_path.join(path);
        match full_path.metadata() {
            Ok(metadata) => Ok(metadata.file_type().is_dir()),
            Err(e) => Err(map_reader_error(e, full_path)),
        }
    }
}

// -----------------------------------------------------------------------------
// FileWriter


impl Writer for File {
    #[inline(always)]
    fn write_all_bytes<'a>(&'a mut self, buf: &'a [u8]) -> WriteAllFuture<'a> {
        WriteAllFuture::async_write::<File>(self, buf)
    }
}


struct FileWriter<'a> {
    file: File,
    #[cfg(windows)]
    _guard: PhantomData<&'a ()>,
    #[cfg(not(windows))]
    _guard: Option<SemaphoreGuard<'a>>,
}

impl AsyncWrite for FileWriter<'_> {
    #[inline(always)]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.file).poll_write(cx, buf)
    }

    #[inline(always)]
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.file).poll_flush(cx)
    }

    #[inline(always)]
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.file).poll_close(cx)
    }
}

impl Writer for FileWriter<'_> {
    #[inline(always)]
    fn write_all_bytes<'a>(&'a mut self, buf: &'a [u8]) -> WriteAllFuture<'a> {
        WriteAllFuture::async_write::<File>(&mut self.file, buf)
    }
}

// -----------------------------------------------------------------------------
// AssetWriter

#[cold]
fn map_write_error(e: std::io::Error, path: PathBuf) -> AssetWriterError {
    use std::io::ErrorKind;
    match e.kind() {
        ErrorKind::NotFound => AssetWriterError::NotFound(path),
        ErrorKind::InvalidFilename => AssetWriterError::InvalidFilename(path),
        ErrorKind::DirectoryNotEmpty => AssetWriterError::DirectoryNotEmpty(path),
        _ => AssetWriterError::Io(e)
    }
}

impl AssetWriter for FileAssetWriter {
    async fn write<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<impl Writer + 'a, AssetWriterError> {
        #[cfg(windows)]
        let _guard = PhantomData;
        #[cfg(not(windows))]
        let _guard = maybe_get_semaphore().await;

        let full_path = self.root_path.join(path);
        if let Some(parent) = full_path.parent() {
            async_fs::create_dir_all(parent).await?;
        }

        match File::create(&full_path).await {
            Ok(file) => Ok(FileWriter { file, _guard }),
            Err(e) => Err(map_write_error(e, full_path)),
        }
    }

    async fn write_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<impl Writer + 'a, AssetWriterError> {
        #[cfg(windows)]
        let _guard = PhantomData;
        #[cfg(not(windows))]
        let _guard = maybe_get_semaphore().await;

        let meta_path = append_meta_extension(path);
        let full_path = self.root_path.join(meta_path);

        if let Some(parent) = full_path.parent() {
            async_fs::create_dir_all(parent).await?;
        }

        match File::create(&full_path).await {
            Ok(file) => Ok(FileWriter { file, _guard }),
            Err(e) => Err(map_write_error(e, full_path)),
        }
    }

    async fn remove<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let full_path = self.root_path.join(path);
        async_fs::remove_file(&full_path).await.map_err(|e|map_write_error(e, full_path))
    }

    async fn remove_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let meta_path = append_meta_extension(path);
        let full_path = self.root_path.join(meta_path);
        async_fs::remove_file(&full_path).await.map_err(|e|map_write_error(e, full_path))
    }

    async fn rename<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let full_old_path = self.root_path.join(old_path);
        let full_new_path = self.root_path.join(new_path);
        if let Some(parent) = full_new_path.parent() {
            async_fs::create_dir_all(parent).await?;
        }
        async_fs::rename(full_old_path, full_new_path).await.map_err(AssetWriterError::Io)
    }

    async fn rename_meta<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let old_meta_path = append_meta_extension(old_path);
        let new_meta_path = append_meta_extension(new_path);
        let full_old_path = self.root_path.join(old_meta_path);
        let full_new_path = self.root_path.join(new_meta_path);
        if let Some(parent) = full_new_path.parent() {
            async_fs::create_dir_all(parent).await?;
        }
        async_fs::rename(full_old_path, full_new_path).await.map_err(AssetWriterError::Io)
    }

    async fn create_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let full_path = self.root_path.join(path);
        async_fs::create_dir_all(&full_path).await.map_err(|e|map_write_error(e, full_path))
    }

    async fn remove_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let full_path = self.root_path.join(path);
        async_fs::remove_dir_all(&full_path).await.map_err(|e|map_write_error(e, full_path))
    }

    async fn remove_empty_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let full_path = self.root_path.join(path);
        async_fs::remove_dir(&full_path).await.map_err(|e|map_write_error(e, full_path))
    }

    async fn remove_assets_in_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let full_path = self.root_path.join(path);
        async_fs::remove_dir_all(&full_path).await?;
        async_fs::create_dir_all(&full_path).await?;
        Ok(())
    }
}


