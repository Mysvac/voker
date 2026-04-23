use core::any::Any;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};

use crate::meta::{DynamicAssetMeta, Settings};

/// Performs a read from the `slice` into `buf`.
pub(crate) fn slice_read(slice: &[u8], bytes_read: &mut usize, buf: &mut [u8]) -> usize {
    if *bytes_read >= slice.len() {
        0
    } else {
        let src = &slice[(*bytes_read)..];

        // See `std::io::Read for &[u8]`
        let amt = core::cmp::min(buf.len(), src.len());

        if amt == 1 {
            buf[0] = src[0];
        } else {
            buf[..amt].copy_from_slice(&src[..amt]);
        }

        *bytes_read += amt;

        amt
    }
}

pub(crate) fn slice_seek(
    slice: &[u8],
    bytes_read: &mut usize,
    pos: SeekFrom,
) -> std::io::Result<u64> {
    #[inline]
    fn make_error() -> std::io::Result<u64> {
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

    let Ok(offset) = offset else {
        return make_error();
    };

    let Ok(origin) = i64::try_from(origin) else {
        return make_error();
    };

    let Ok(new_pos) = usize::try_from(origin + offset) else {
        return make_error();
    };

    *bytes_read = new_pos;
    Ok(new_pos as u64)
}

/// Appends `.meta` to the given path:
/// - `foo` becomes `foo.meta`
/// - `foo.bar` becomes `foo.bar.meta`
pub(crate) fn build_meta_path(path: &Path) -> PathBuf {
    let mut meta_path = path.to_path_buf();
    let mut extension = path.extension().unwrap_or_default().to_os_string();
    if !extension.is_empty() {
        extension.push(".");
    }
    extension.push("meta");
    meta_path.set_extension(extension);
    meta_path
}

pub(crate) fn meta_transform_settings<S: Settings>(
    meta: &mut dyn DynamicAssetMeta,
    settings: &(impl Fn(&mut S) + Send + Sync + 'static),
) {
    if let Some(loader_settings) = meta.loader_settings_mut() {
        if let Some(loader_settings) = <dyn Any>::downcast_mut::<S>(loader_settings) {
            settings(loader_settings);
        } else {
            tracing::error!(
                "Configured settings type {} does not match AssetLoader settings type",
                core::any::type_name::<S>(),
            );
        }
    }
}
