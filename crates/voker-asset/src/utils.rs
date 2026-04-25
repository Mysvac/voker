use alloc::boxed::Box;
use core::any::Any;
use core::pin::Pin;
use std::ffi::OsString;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};

use futures_lite::Stream;

use crate::meta::{DynamicAssetMeta, MetaTransform, Settings};

// -----------------------------------------------------------------------------
// Alias

pub type BoxedFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub type PathStream = dyn Stream<Item = PathBuf> + Unpin + Send;

/// A [`PathBuf`] [`Stream`] implementation that immediately returns nothing.
pub struct EmptyPathStream;

impl Stream for EmptyPathStream {
    type Item = PathBuf;
    #[inline(always)]
    fn poll_next(
        self: Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Option<Self::Item>> {
        core::task::Poll::Ready(None)
    }
}

// -----------------------------------------------------------------------------
// Helper

/// Performs a read from the `slice` into `buf`.
#[inline]
pub(crate) fn slice_read(slice: &[u8], bytes_read: &mut usize, buf: &mut [u8]) -> usize {
    if *bytes_read >= slice.len() {
        0
    } else {
        let src = &slice[(*bytes_read)..];

        // See `std::io::Read for &[u8]`
        let amt = core::cmp::min(buf.len(), src.len());

        // The boundary check is automatically eliminated in O3 optimization.
        if amt == 1 {
            buf[0] = src[0];
        } else {
            buf[..amt].copy_from_slice(&src[..amt]);
        }

        *bytes_read += amt;

        amt
    }
}

/// Calculate the position of Seek.
///
/// Return error if the result is out of range (e.g. `< 0` or `> usize::MAX`).
#[inline]
pub(crate) fn slice_seek(
    slice: &[u8],
    bytes_read: &mut usize,
    pos: SeekFrom,
) -> std::io::Result<u64> {
    #[cold]
    #[inline(never)]
    fn make_overflow_error() -> std::io::Result<u64> {
        // TODO: Waiting unstable API `const_error!`.
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "seek position is out of range",
        ))
    }

    let (origin, offset) = match pos {
        SeekFrom::Current(offset) => (*bytes_read, Ok(offset)),
        SeekFrom::Start(offset) => (0, offset.try_into()),
        SeekFrom::End(offset) => (slice.len(), Ok(offset)),
    };

    if let Ok(offset_i64) = offset
        && let Ok(origin_i64) = i64::try_from(origin)
        && let Some(new_pos_i64) = origin_i64.checked_add(offset_i64)
        && let Ok(new_pos) = usize::try_from(new_pos_i64)
    {
        *bytes_read = new_pos;
        Ok(new_pos as u64)
    } else {
        make_overflow_error()
    }
}

/// Appends `.meta` to the given path:
/// - `foo` becomes `foo.meta`
/// - `foo.bar` becomes `foo.bar.meta`
pub(crate) fn append_meta_extension(path: &Path) -> PathBuf {
    let mut meta_path = path.to_path_buf();
    let extension_str = path.extension().unwrap_or_default();
    // Directly `to_os_string` will cause a additional reallocation.
    let mut extension = OsString::with_capacity(extension_str.len() + 5);
    extension.push(extension_str);
    if !extension.is_empty() {
        extension.push(".");
    }
    extension.push("meta");
    meta_path.set_extension(extension);
    meta_path
}

/// Transform the loader setting of given meta through given function.
///
/// No-op if the meta does not contains loader setting.
///
/// Log error if the loader setting's type mismatch the function param.
#[inline]
pub(crate) fn transform_loader_settings<S: Settings>(
    meta: &mut dyn DynamicAssetMeta,
    transform: &(impl Fn(&mut S) + Send + Sync + 'static),
) {
    if let Some(loader_settings) = meta.loader_settings_mut() {
        if let Some(loader_settings) = <dyn Any>::downcast_mut::<S>(loader_settings) {
            transform(loader_settings);
        } else {
            core::hint::cold_path();
            tracing::error!(
                "Configured settings type {} does not match AssetLoader settings type, skipped.",
                core::any::type_name::<S>(),
            );
        }
    }
}

/// Create a `MetaTransform` from given function.
pub(crate) fn wrap_settings_transform<S: Settings>(
    settings: impl Fn(&mut S) + Send + Sync + 'static,
) -> MetaTransform {
    Box::new(move |meta| transform_loader_settings(meta, &settings))
}

/// Bind the transform statements in sequence.
pub(crate) fn bind_settings_transform<S: Settings>(
    prev_transform: MetaTransform,
    settings: impl Fn(&mut S) + Send + Sync + 'static,
) -> MetaTransform {
    Box::new(move |meta| {
        prev_transform(meta);
        transform_loader_settings(meta, &settings);
    })
}

/// Normalizes the path by collapsing all occurrences of '.' and '..' dot-segments
/// where possible as per [RFC 1808](https://datatracker.ietf.org/doc/html/rfc1808)
pub(crate) fn normalize_path(path: &Path) -> PathBuf {
    let size_hint = path.as_os_str().len();
    let mut result_path = PathBuf::with_capacity(size_hint);
    for elt in path.iter() {
        if elt == "." {
            // Skip
        } else if elt == ".." {
            // Note: If the result_path ends in `..`, Path::file_name returns None,
            // so we'll end up preserving it.
            if result_path.file_name().is_some() {
                // This assert is just a sanity check - we already know the path
                // has a file_name, so we know there is something to pop.
                assert!(result_path.pop());
            } else {
                // Preserve ".." if insufficient matches (per RFC 1808).
                result_path.push(elt);
            }
        } else {
            result_path.push(elt);
        }
    }
    result_path
}

/// Return a iterator that contains all secondary extensions.
///
/// For example the full extension is `.a.b.c`,
/// then the returned iterator contains: `a.b.c`, `b.c` and `c`.
pub(crate) fn iter_secondary_extensions(full_extension: &str) -> impl Iterator<Item = &str> {
    full_extension.char_indices().filter_map(|(i, c)| {
        if c == '.' {
            Some(&full_extension[i + 1..])
        } else {
            None
        }
    })
}
