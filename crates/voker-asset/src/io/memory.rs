use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::pin::Pin;
use core::task::Poll;
use futures_io::{AsyncRead, AsyncSeek, AsyncWrite};
use futures_lite::Stream;
use std::path::{Path, PathBuf};

use voker_os::sync::Arc;
use voker_os::sync::{PoisonError, RwLock};
use voker_utils::hash::HashMap;
use voker_utils::vec::FastVec;

use crate::PathStream;
use crate::io::{AssetWriter, AssetWriterError, Writer};

use super::{AssetReader, AssetReaderError};
use super::{Reader, ReaderNotSeekableError, SeekableReader};

// -----------------------------------------------------------------------------
// Value

#[derive(Clone)]
pub enum Value {
    Borrow(Arc<Vec<u8>>),
    Static(&'static [u8]),
}

impl Debug for Value {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Borrow(arg) => f
                .debug_struct("Value::Borrow")
                .field("ptr", &<[u8]>::as_ptr(arg))
                .field("len", &<[u8]>::len(arg))
                .finish(),
            Self::Static(arg) => f
                .debug_struct("Value::Static")
                .field("ptr", &<[u8]>::as_ptr(arg))
                .field("len", &<[u8]>::len(arg))
                .finish(),
        }
    }
}

impl From<Vec<u8>> for Value {
    #[inline]
    fn from(value: Vec<u8>) -> Self {
        Self::Borrow(Arc::new(value))
    }
}

impl From<Arc<Vec<u8>>> for Value {
    #[inline]
    fn from(value: Arc<Vec<u8>>) -> Self {
        Self::Borrow(value)
    }
}

impl From<&'static [u8]> for Value {
    #[inline]
    fn from(value: &'static [u8]) -> Self {
        Self::Static(value)
    }
}

impl<const N: usize> From<&'static [u8; N]> for Value {
    #[inline]
    fn from(value: &'static [u8; N]) -> Self {
        Self::Static(value)
    }
}

// -----------------------------------------------------------------------------
// Data

#[derive(Clone, Debug)]
pub struct Data {
    path: PathBuf,
    value: Value,
}

impl Data {
    /// The path that this data was written to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The value in bytes that was written here.
    pub fn value(&self) -> &[u8] {
        match &self.value {
            Value::Borrow(vec) => vec,
            Value::Static(value) => value,
        }
    }
}

// -----------------------------------------------------------------------------
// Dir

#[derive(Default, Debug)]
struct DirInternal {
    assets: HashMap<Box<str>, Data>,
    metadata: HashMap<Box<str>, Data>,
    dirs: HashMap<Box<str>, Dir>,
    path: PathBuf,
}

#[derive(Default, Clone, Debug)]
pub struct Dir(Arc<RwLock<DirInternal>>);

impl Dir {
    pub fn new(path: PathBuf) -> Self {
        Self(Arc::new(RwLock::new(DirInternal {
            path,
            assets: HashMap::new(),
            metadata: HashMap::new(),
            dirs: HashMap::new(),
        })))
    }

    #[inline]
    pub fn validate_dir_path(path: &Path) -> bool {
        path.components()
            .all(|c| !matches!(c, std::path::Component::Prefix(_)))
    }

    #[inline]
    pub fn validate_file_path(path: &Path) -> bool {
        if let Some(parent) = path.parent() {
            Dir::validate_dir_path(parent) && path.file_name().is_some()
        } else {
            path.file_name().is_some()
        }
    }

    /// - Get or create a dir.
    /// - `self` must be the Root Dir, otherwise it's UB (the path of new dir may incorrect).
    /// - `.` and `..` is invalid, but non-existent parent will be ignore and log a warning.
    /// - the `path` should not contains `Prefix` (e.g. `C:`), otherwise this will panic.
    pub fn resolve_dir(&self, path: &Path) -> Dir {
        let mut dir = self.clone();

        let size_hint = path.as_os_str().len();

        let mut full_path = PathBuf::with_capacity(size_hint);
        let mut buffer = FastVec::<Dir, 6>::new();
        let data = buffer.data();

        for c in path.components() {
            match c {
                std::path::Component::CurDir => continue,
                std::path::Component::RootDir => {
                    data.clear();
                    full_path.clear();
                    dir = self.clone();
                    continue;
                }
                std::path::Component::ParentDir => {
                    // We cannot add reverse edges, as this
                    // would cause circular references.
                    if let Some(parent) = data.pop() {
                        dir = parent;
                    } else {
                        tracing::warn!(
                            "Parent directory is non-existent, ignoring '..' component: `{}`.",
                            path.display()
                        );
                    }
                    continue;
                }
                std::path::Component::Normal(osstr) => {
                    full_path.push(c);
                    data.push(dir.clone());
                    let name: Box<str> = osstr.to_string_lossy().into();
                    let next_dir = dir
                        .0
                        .write()
                        .unwrap_or_else(PoisonError::into_inner)
                        .dirs
                        .entry(name)
                        .or_insert_with(|| Dir::new(full_path.clone()))
                        .clone();
                    dir = next_dir;
                }
                _ => {
                    core::hint::cold_path();
                    unreachable!("Path Prefix is unsupported: `{}`.", path.display())
                }
            }
        }

        dir
    }

    /// - `self` must be the Root Dir, otherwise it's UB (the path of new dir may incorrect).
    /// - the `path` should not contains `Prefix` (e.g. `C:`), otherwise this will panic.
    /// - the `path` should contains file_name, otherwise this will panic.
    pub fn insert_asset(&self, path: &Path, value: impl Into<Value>) {
        let mut dir = self.clone();

        if let Some(parent) = path.parent() {
            dir = self.resolve_dir(parent);
        }

        // Separate to reduce the lock occupation time.
        let name: Box<str> = path.file_name().unwrap().to_string_lossy().into();
        let path: PathBuf = path.to_owned();
        let value: Value = value.into();
        let data: Data = Data { value, path };

        dir.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .assets
            .insert(name, data);
    }

    /// - `self` must be the Root Dir, otherwise it's UB (the path of new dir may incorrect).
    /// - the `path` should not contains `Prefix` (e.g. `C:`), otherwise this will panic.
    /// - the `path` should contains file_name, otherwise this will panic.
    pub fn insert_meta(&self, path: &Path, value: impl Into<Value>) {
        let mut dir = self.clone();

        if let Some(parent) = path.parent() {
            dir = self.resolve_dir(parent);
        }

        let name: Box<str> = path.file_name().unwrap().to_string_lossy().into();
        let path: PathBuf = path.to_owned();
        let value: Value = value.into();
        let data: Data = Data { value, path };

        dir.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .metadata
            .insert(name, data);
    }

    /// - `self` must be the Root Dir, otherwise it's UB (the path of new dir may incorrect).
    /// - the `path` should not contains `Prefix` (e.g. `C:`), otherwise this will panic.
    /// - the `path` should contains file_name, otherwise this will panic.
    pub fn insert_asset_text(&self, path: &Path, asset: &str) {
        self.insert_asset(path, asset.as_bytes().to_vec());
    }

    /// - `self` must be the Root Dir, otherwise it's UB (the path of new dir may incorrect).
    /// - the `path` should not contains `Prefix` (e.g. `C:`), otherwise this will panic.
    /// - the `path` should contains file_name, otherwise this will panic.
    pub fn insert_meta_text(&self, path: &Path, asset: &str) {
        self.insert_meta(path, asset.as_bytes().to_vec());
    }

    /// - self can be not-root dir, then the input should be relative path.
    /// - Return `None` if path is invalid, will not panic.
    pub fn remove_asset(&self, path: &Path) -> Option<Data> {
        let mut dir = self.clone();

        if let Some(parent) = path.parent() {
            dir = self.get_dir(parent)?;
        }

        let name: &str = path.file_name()?.to_str()?;
        dir.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .assets
            .remove(name)
    }

    /// - self can be not-root dir, then the input should be relative path.
    /// - Return `None` if path is invalid, will not panic.
    pub fn remove_meta(&self, path: &Path) -> Option<Data> {
        let mut dir = self.clone();
        if let Some(parent) = path.parent() {
            dir = self.get_dir(parent)?;
        }

        let name: &str = path.file_name()?.to_str()?;
        dir.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .metadata
            .remove(name)
    }

    /// - self can be not-root dir, then the input should be relative path.
    /// - Return `None` if path is invalid, will not panic.
    pub fn remove_dir(&self, path: &Path) -> Option<Dir> {
        let mut dir = self.clone();
        if let Some(parent) = path.parent() {
            dir = self.resolve_dir(parent);
        }

        let name: &str = path.file_name()?.to_str()?;
        dir.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .dirs
            .remove(name)
    }

    /// - self can be not-root dir, then the input should be relative path.
    /// - Return `None` if path is invalid, will not panic.
    pub fn get_dir(&self, path: &Path) -> Option<Dir> {
        let mut dir = self.clone();

        let mut buffer = FastVec::<Dir, 6>::new();
        let data = buffer.data();

        for c in path.components() {
            match c {
                std::path::Component::CurDir => continue,
                std::path::Component::RootDir => {
                    data.clear();
                    dir = self.clone();
                    continue;
                }
                std::path::Component::ParentDir => {
                    if let Some(parent) = data.pop() {
                        dir = parent;
                    } else {
                        return None;
                    }
                    continue;
                }
                std::path::Component::Normal(osstr) => {
                    let name = osstr.to_str().unwrap();
                    let next_dir = dir
                        .0
                        .read()
                        .unwrap_or_else(PoisonError::into_inner)
                        .dirs
                        .get(name)?
                        .clone();
                    dir = next_dir;
                }
                _ => {
                    core::hint::cold_path();
                    return None;
                }
            }
        }

        Some(dir)
    }

    /// - self can be not-root dir, then the input should be relative path.
    /// - Return `None` if path is invalid, will not panic.
    pub fn get_asset(&self, path: &Path) -> Option<Data> {
        let mut dir = self.clone();

        if let Some(parent) = path.parent() {
            dir = dir.get_dir(parent)?;
        }

        let name: &str = path.file_name()?.to_str()?;
        dir.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .assets
            .get(name)
            .cloned()
    }

    /// - self can be not-root dir, then the input should be relative path.
    /// - Return `None` if path is invalid, will not panic.
    pub fn get_meta(&self, path: &Path) -> Option<Data> {
        let mut dir = self.clone();

        if let Some(parent) = path.parent() {
            dir = dir.get_dir(parent)?;
        }

        let name: &str = path.file_name()?.to_str()?;
        dir.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .metadata
            .get(name)
            .cloned()
    }

    pub fn path(&self) -> PathBuf {
        self.0.read().unwrap_or_else(PoisonError::into_inner).path.clone()
    }
}

// -----------------------------------------------------------------------------
// DirStream

pub struct DirStream {
    dir: Dir,
    index: usize,
    dir_index: usize,
}

impl DirStream {
    fn new(dir: Dir) -> Self {
        Self {
            dir,
            index: 0,
            dir_index: 0,
        }
    }
}

impl Stream for DirStream {
    type Item = PathBuf;

    fn poll_next(
        self: Pin<&mut Self>,
        _ctx: &mut core::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let dir = this.dir.0.read().unwrap_or_else(PoisonError::into_inner);

        let dir_index = this.dir_index;
        let dir_path = dir.dirs.keys().nth(dir_index).map(|d| dir.path.join(d.as_ref()));

        if let Some(dir_path) = dir_path {
            this.dir_index += 1;
            Poll::Ready(Some(dir_path))
        } else {
            let index = this.index;
            this.index += 1;
            let data = dir.assets.values().nth(index);
            Poll::Ready(data.map(|d| d.path().to_owned()))
        }
    }
}

// -----------------------------------------------------------------------------
// DataReader

struct DataReader {
    data: Data,
    bytes_read: usize,
}

impl AsyncRead for DataReader {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<futures_io::Result<usize>> {
        use crate::utils::slice_read;
        let this = self.get_mut();
        let slice = this.data.value();
        let bytes_read = &mut this.bytes_read;
        Poll::Ready(Ok(slice_read(slice, bytes_read, buf)))
    }
}

impl AsyncSeek for DataReader {
    #[inline]
    fn poll_seek(
        self: Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
        pos: std::io::SeekFrom,
    ) -> Poll<std::io::Result<u64>> {
        use crate::utils::slice_seek;
        let this = self.get_mut();
        let slice = this.data.value();
        let bytes_read = &mut this.bytes_read;
        Poll::Ready(slice_seek(slice, bytes_read, pos))
    }
}

impl Reader for DataReader {
    #[inline(always)]
    fn seekable(&mut self) -> Result<&mut dyn SeekableReader, ReaderNotSeekableError> {
        Ok(self)
    }
}

// -----------------------------------------------------------------------------
// MemoryAssetReader

#[derive(Default, Clone)]
pub struct MemoryAssetReader {
    pub root: Dir,
}

impl AssetReader for MemoryAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        match self.root.get_asset(path) {
            Some(data) => Ok(DataReader {
                data,
                bytes_read: 0,
            }),
            None => Err(AssetReaderError::NotFound(path.to_path_buf())),
        }
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        match self.root.get_meta(path) {
            Some(data) => Ok(DataReader {
                data,
                bytes_read: 0,
            }),
            None => Err(AssetReaderError::NotFound(path.to_path_buf())),
        }
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        match self.root.get_dir(path) {
            Some(dir) => Ok(Box::new(DirStream::new(dir))),
            None => Err(AssetReaderError::NotFound(path.to_path_buf())),
        }
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(self.root.get_dir(path).is_some())
    }
}

// -----------------------------------------------------------------------------
// DataWriter

struct DataWriter {
    /// The dir to write to.
    dir: Dir,
    /// The path to write to.
    path: PathBuf,
    /// The current buffer of data.
    ///
    /// This will include data that has been flushed already.
    current_data: Vec<u8>,
    /// Whether to write to the data or to the meta.
    is_meta_writer: bool,
}

impl AsyncWrite for DataWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        this.current_data.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut core::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        let value = self.current_data.clone();
        let path = &self.path;
        if self.is_meta_writer {
            self.dir.insert_meta(path, value);
        } else {
            self.dir.insert_asset(path, value);
        }
        Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.poll_flush(cx)
    }
}

impl Writer for DataWriter {}

// -----------------------------------------------------------------------------
// MemoryAssetWriter

#[derive(Default, Clone)]
pub struct MemoryAssetWriter {
    pub root: Dir,
}

impl AssetWriter for MemoryAssetWriter {
    async fn write<'a>(&'a self, path: &'a Path) -> Result<impl Writer + 'a, AssetWriterError> {
        if !Dir::validate_file_path(path) {
            core::hint::cold_path();
            return Err(AssetWriterError::InvalidPath(path.to_path_buf()));
        }
        Ok(DataWriter {
            dir: self.root.clone(),
            path: path.to_owned(),
            current_data: Vec::new(),
            is_meta_writer: false,
        })
    }

    async fn write_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<impl Writer + 'a, AssetWriterError> {
        if !Dir::validate_file_path(path) {
            core::hint::cold_path();
            return Err(AssetWriterError::InvalidPath(path.to_path_buf()));
        }
        Ok(DataWriter {
            dir: self.root.clone(),
            path: path.to_owned(),
            current_data: Vec::new(),
            is_meta_writer: true,
        })
    }

    async fn remove<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        if self.root.remove_asset(path).is_none() {
            // TODO: Waiting for stable macro `std::io::const_error!`
            let error = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
            return Err(AssetWriterError::from(error));
        }
        Ok(())
    }

    async fn remove_meta<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        if self.root.remove_meta(path).is_none() {
            // TODO: Waiting for stable macro `std::io::const_error!`
            let error = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
            return Err(AssetWriterError::from(error));
        }
        Ok(())
    }

    async fn rename<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let Some(old_asset) = self.root.get_asset(old_path) else {
            // TODO: Waiting for stable macro `std::io::const_error!`
            let error = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
            return Err(AssetWriterError::from(error));
        };
        if old_path == new_path {
            return Ok(());
        }
        self.root.insert_asset(new_path, old_asset.value);
        // Remove the asset after instead of before since otherwise there'd be a
        // moment where the Dir is unlocked and missing both the old and new paths.
        self.root.remove_asset(old_path);
        Ok(())
    }

    async fn rename_meta<'a>(
        &'a self,
        old_path: &'a Path,
        new_path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let Some(old_asset) = self.root.get_meta(old_path) else {
            // TODO: Waiting for stable macro `std::io::const_error!`
            let error = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
            return Err(AssetWriterError::from(error));
        };
        if old_path == new_path {
            return Ok(());
        }
        self.root.insert_meta(new_path, old_asset.value);
        // Remove the meta after instead of before since otherwise there'd be a
        // moment where the Dir is unlocked and missing both the old and new paths.
        self.root.remove_meta(old_path);
        Ok(())
    }

    async fn create_directory<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        if !Dir::validate_dir_path(path) {
            core::hint::cold_path();
            return Err(AssetWriterError::InvalidPath(path.to_path_buf()));
        }
        self.root.resolve_dir(path);
        Ok(())
    }

    async fn remove_directory<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        if self.root.remove_dir(path).is_none() {
            // TODO: Waiting for stable macro `std::io::const_error!`
            let error = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
            return Err(AssetWriterError::from(error));
        }
        Ok(())
    }

    async fn remove_empty_directory<'a>(&'a self, path: &'a Path) -> Result<(), AssetWriterError> {
        let Some(dir) = self.root.get_dir(path) else {
            // TODO: Waiting for stable macro `std::io::const_error!`
            let error = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
            return Err(AssetWriterError::from(error));
        };

        let dir = dir.0.read().unwrap();

        if !dir.assets.is_empty() || !dir.metadata.is_empty() || !dir.dirs.is_empty() {
            // TODO: Waiting for stable macro `std::io::const_error!`
            let error = std::io::Error::new(std::io::ErrorKind::NotFound, "not empty");
            return Err(AssetWriterError::from(error));
        }

        self.root.remove_dir(path);
        Ok(())
    }

    async fn remove_assets_in_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<(), AssetWriterError> {
        let Some(dir) = self.root.get_dir(path) else {
            // TODO: Waiting for stable macro `std::io::const_error!`
            let error = std::io::Error::new(std::io::ErrorKind::NotFound, "no such dir");
            return Err(AssetWriterError::from(error));
        };

        let mut dir = dir.0.write().unwrap();

        dir.assets.clear();
        dir.dirs.clear();
        dir.metadata.clear();
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
pub mod test {
    use super::Dir;
    use std::path::Path;

    #[test]
    fn memory_dir() {
        let dir = Dir::default();
        let a_path = Path::new("a.txt");
        let a_data = "a".as_bytes().to_vec();
        let a_meta = "ameta".as_bytes().to_vec();

        dir.insert_asset(a_path, a_data.clone());
        let asset = dir.get_asset(a_path).unwrap();
        assert_eq!(asset.path(), a_path);
        assert_eq!(asset.value(), a_data);

        dir.insert_meta(a_path, a_meta.clone());
        let meta = dir.get_meta(a_path).unwrap();
        assert_eq!(meta.path(), a_path);
        assert_eq!(meta.value(), a_meta);

        let b_path = Path::new("x/y/b.txt");
        let b_data = "b".as_bytes().to_vec();
        let b_meta = "meta".as_bytes().to_vec();
        dir.insert_asset(b_path, b_data.clone());
        dir.insert_meta(b_path, b_meta.clone());

        let asset = dir.get_asset(b_path).unwrap();
        assert_eq!(asset.path(), b_path);
        assert_eq!(asset.value(), b_data);

        let meta = dir.get_meta(b_path).unwrap();
        assert_eq!(meta.path(), b_path);
        assert_eq!(meta.value(), b_meta);
    }
}
