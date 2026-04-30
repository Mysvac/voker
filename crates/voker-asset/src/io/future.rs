#![expect(unsafe_code, reason = "pointer operation")]

use alloc::vec::Vec;
use core::pin::Pin;
use core::ptr::NonNull;
use core::task::{Context, Poll};

use futures_io::{AsyncRead, AsyncWrite};
use futures_lite::ready;

// -----------------------------------------------------------------------------
// ReadAllFunc

#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadAllFuture<'a> {
    func: fn(Pin<&mut Self>, &mut Context) -> Poll<std::io::Result<usize>>,
    reader: NonNull<u8>,
    buffer: &'a mut Vec<u8>,
    data_size: usize,
    start_len: usize,
}

impl Unpin for ReadAllFuture<'_> {}
unsafe impl Send for ReadAllFuture<'_> {}
unsafe impl Sync for ReadAllFuture<'_> {}

impl Future for ReadAllFuture<'_> {
    type Output = std::io::Result<usize>;

    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        (self.func)(self, cx)
    }
}

impl ReadAllFuture<'_> {
    #[inline(always)]
    pub fn async_read<'a, R: AsyncRead + Send + Sync + Unpin>(
        reader: &'a mut R,
        output: &'a mut Vec<u8>,
    ) -> ReadAllFuture<'a> {
        let start_len = output.len();
        ReadAllFuture {
            func: async_read_internal::<R>,
            reader: NonNull::from_mut(reader).cast(),
            buffer: output,
            start_len,
            data_size: 0, // unused
        }
    }

    #[inline(always)]
    pub fn slice_read<'a>(reader: &'a [u8], output: &'a mut Vec<u8>) -> ReadAllFuture<'a> {
        let start_len = output.len();
        let data_size = reader.len();
        ReadAllFuture {
            func: slice_read_internal,
            reader: NonNull::from_ref(reader).cast(),
            buffer: output,
            start_len, // bitflag
            data_size,
        }
    }
}

// From `future_lite::AsyncReadExt::read_to_end`
fn async_read_internal<R: AsyncRead + Unpin>(
    this: Pin<&mut ReadAllFuture>,
    cx: &mut Context<'_>,
) -> Poll<std::io::Result<usize>> {
    let ReadAllFuture {
        reader,
        buffer,
        start_len,
        ..
    } = this.get_mut();

    let start_len = *start_len;
    let reader: &mut R = unsafe { reader.cast::<R>().as_mut() };
    let buf: &mut Vec<u8> = buffer;
    let mut rd: Pin<&mut R> = Pin::new(reader);

    struct Guard<'a> {
        buf: &'a mut Vec<u8>,
        len: usize,
    }

    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            self.buf.resize(self.len, 0);
        }
    }

    let mut g = Guard {
        len: buf.len(),
        buf,
    };

    let ret;

    loop {
        if g.len == g.buf.len() {
            g.buf.reserve(32);
            let capacity = g.buf.capacity();
            // Faster then `resize(capacity, 0)`, no need to reset memory.
            unsafe { g.buf.set_len(capacity) };
        }

        match ready!(rd.as_mut().poll_read(cx, &mut g.buf[g.len..])) {
            Ok(0) => {
                ret = Poll::Ready(Ok(g.len - start_len));
                break;
            }
            Ok(n) => g.len += n,
            Err(e) => {
                ret = Poll::Ready(Err(e));
                break;
            }
        }
    }

    ret
}

fn slice_read_internal(
    this: Pin<&mut ReadAllFuture>,
    _cx: &mut Context<'_>,
) -> Poll<std::io::Result<usize>> {
    // we separate it to optimize branch prediction.
    #[cold]
    #[inline(never)]
    fn overflow() -> std::io::Error {
        std::io::ErrorKind::FileTooLarge.into()
    }

    let ReadAllFuture {
        reader,
        buffer,
        data_size,
        start_len,
        ..
    } = this.get_mut();

    let start_len: usize = *start_len;
    let data_size: usize = *data_size;
    let buf: &mut Vec<u8> = buffer;

    let result_len = match data_size.checked_add(start_len) {
        Some(new) if new < isize::MAX as usize => new,
        _ => return Poll::Ready(Err(overflow())),
    };

    let old_cap = buf.capacity();
    let additional = result_len.saturating_sub(old_cap);
    buf.reserve(additional);

    unsafe {
        let src = reader.as_ptr();
        let dst = buf.as_mut_ptr().add(start_len);
        core::ptr::copy_nonoverlapping(src, dst, data_size);
        buf.set_len(result_len);
    }

    Poll::Ready(Ok(data_size))
}

// -----------------------------------------------------------------------------
// ReadAllFunc

#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct WriteAllFuture<'a> {
    func: fn(Pin<&mut Self>, &mut Context) -> Poll<std::io::Result<()>>,
    writer: NonNull<u8>,
    buffer: &'a [u8],
}

impl Unpin for WriteAllFuture<'_> {}
unsafe impl Send for WriteAllFuture<'_> {}
unsafe impl Sync for WriteAllFuture<'_> {}

impl Future for WriteAllFuture<'_> {
    type Output = std::io::Result<()>;

    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        (self.func)(self, cx)
    }
}

impl WriteAllFuture<'_> {
    #[inline(always)]
    pub fn async_write<'a, R: AsyncWrite + Send + Sync + Unpin>(
        writer: &'a mut R,
        input: &'a [u8],
    ) -> WriteAllFuture<'a> {
        WriteAllFuture {
            func: async_write_internal::<R>,
            writer: NonNull::from_mut(writer).cast(),
            buffer: input,
        }
    }

    #[inline(always)]
    pub fn vec_write<'a>(writer: &'a mut Vec<u8>, input: &'a [u8]) -> WriteAllFuture<'a> {
        WriteAllFuture {
            func: vec_write_internal,
            writer: NonNull::from_mut(writer).cast(),
            buffer: input,
        }
    }
}

// From `future_lite::AsyncWriteExt::write_all`
fn async_write_internal<R: AsyncWrite + Unpin>(
    this: Pin<&mut WriteAllFuture>,
    cx: &mut Context<'_>,
) -> Poll<std::io::Result<()>> {
    let WriteAllFuture { writer, buffer, .. } = this.get_mut();

    while !buffer.is_empty() {
        let writer: &mut R = unsafe { writer.cast::<R>().as_mut() };
        let writer: Pin<&mut R> = Pin::new(writer);

        let n = ready!(writer.poll_write(cx, buffer))?;
        // Incorrect length may cause a panic, so we temporarily replace
        // the buffer to ensure that the panic is considered complete.
        let taked = core::mem::take(buffer);
        *buffer = taked.split_at(n).1;

        if n == 0 {
            return Poll::Ready(Err(std::io::ErrorKind::WriteZero.into()));
        }
    }

    Poll::Ready(Ok(()))
}

fn vec_write_internal(
    this: Pin<&mut WriteAllFuture>,
    _cx: &mut Context<'_>,
) -> Poll<std::io::Result<()>> {
    // we separate it to optimize branch prediction.
    #[cold]
    #[inline(never)]
    fn overflow() -> std::io::Error {
        std::io::ErrorKind::FileTooLarge.into()
    }

    let WriteAllFuture { writer, buffer, .. } = this.get_mut();

    if !buffer.is_empty() {
        let writer: &mut Vec<u8> = unsafe { writer.cast::<Vec<u8>>().as_mut() };

        let start_len = writer.len();
        let data_size = buffer.len();

        let result_len = match data_size.checked_add(start_len) {
            Some(new) if new < isize::MAX as usize => new,
            _ => return Poll::Ready(Err(overflow())),
        };

        writer.reserve(data_size);

        unsafe {
            let taked = core::mem::take(buffer);
            let src = taked.as_ptr();
            let dst = writer.as_mut_ptr().add(start_len);
            core::ptr::copy_nonoverlapping(src, dst, data_size);
            writer.set_len(result_len);
        }
    }

    Poll::Ready(Ok(()))
}
