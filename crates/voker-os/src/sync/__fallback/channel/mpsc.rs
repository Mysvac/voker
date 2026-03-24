//! Fallback implementation of `mpsc` from the standard library.
//!
//! Multi-producer, single-consumer FIFO queue communication primitives.
//!
//! This module provides message-based communication over channels, concretely
//! defined among three types:
//!
//! - [`Sender`]
//! - [`SyncSender`]
//! - [`Receiver`]
//!
//! Internally, `SyncSender` is implemented with a fixed-length ring buffer (circular array).
//! In contrast, `Sender` utilizes a `VecDeque` that automatically shrinks its capacity
//! when it becomes significantly larger than the actual number of stored elements.
//!
//! See the [standard library] for further details.
//!
//! [standard library]: https://doc.rust-lang.org/std/sync/mpsc/index.html

use core::{fmt, marker::PhantomData, panic::RefUnwindSafe, time::Duration};

use crate::sync::Arc;

use super::internal::{ArrayChannel, ListChannel};

pub use super::error::{RecvError, RecvTimeoutError, SendError, TryRecvError, TrySendError};

/// Fallback implementation of `Sender` from the standard library.
///
/// The sending-half of Rust's asynchronous [`channel`] type.
///
/// See the [standard library] for further details.
///
/// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/struct.Sender.html
pub struct Sender<T> {
    inner: Arc<ListChannel<T>>,
}

/// Fallback implementation of `SyncSender` from the standard library.
///
/// The sending-half of Rust's asynchronous [`sync_channel`] type.
///
/// See the [standard library] for further details.
///
/// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/struct.SyncSender.html
pub struct SyncSender<T> {
    inner: Arc<ArrayChannel<T>>,
}

enum InnerReceiver<T> {
    List(Arc<ListChannel<T>>),
    Array(Arc<ArrayChannel<T>>),
}

/// Fallback implementation of `Receiver` from the standard library.
///
/// The receiving half of Rust's [`channel`] (or [`sync_channel`]) type.
/// This half can only be owned by one thread.
///
/// See the [standard library] for further details.
///
/// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html
pub struct Receiver<T> {
    inner: InnerReceiver<T>,
    _marker: PhantomData<*const ()>, // !Sync
}

#[expect(unsafe_code, reason = "impl Send for Receiver")]
unsafe impl<T: Send> Send for Receiver<T> {}
impl<T: RefUnwindSafe> RefUnwindSafe for Receiver<T> {}

/// Creates a new asynchronous channel, returning the sender/receiver halves.
///
/// All data sent on the [`Sender`] will become available on the [`Receiver`] in
/// the same order as it was sent, and no [`send`] will block the calling thread
/// (this channel has an "infinite buffer", unlike [`sync_channel`], which will
/// block after its buffer limit is reached). [`recv`] will block until a message
/// is available while there is at least one [`Sender`] alive (including clones).
///
/// The [`Sender`] can be cloned to [`send`] to the same channel multiple times, but
/// only one [`Receiver`] is supported.
///
/// If the [`Receiver`] is disconnected while trying to [`send`] with the
/// [`Sender`], the [`send`] method will return a [`SendError`]. Similarly, if the
/// [`Sender`] is disconnected while trying to [`recv`], the [`recv`] method will
/// return a [`RecvError`].
///
/// See the [standard library] for further details.
///
/// [`send`]: Sender::send
/// [`recv`]: Receiver::recv
/// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/fn.channel.html
#[must_use]
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let channal = Arc::new(ListChannel::<T>::new());
    let cloned = channal.clone();
    (
        Sender { inner: channal },
        Receiver {
            inner: InnerReceiver::List(cloned),
            _marker: PhantomData,
        },
    )
}

/// Creates a new synchronous, bounded channel.
///
/// All data sent on the [`SyncSender`] will become available on the [`Receiver`]
/// in the same order as it was sent. Like asynchronous [`channel`]s, the
/// [`Receiver`] will block until a message becomes available. `sync_channel`
/// differs greatly in the semantics of the sender, however.
///
/// This channel has an internal buffer on which messages will be queued.
/// `bound` specifies the buffer size. When the internal buffer becomes full,
/// future sends will *block* waiting for the buffer to open up. Note that a
/// buffer size of 0 is valid, in which case this becomes "rendezvous channel"
/// where each [`send`] will not return until a [`recv`] is paired with it.
///
/// The [`SyncSender`] can be cloned to [`send`] to the same channel multiple
/// times, but only one [`Receiver`] is supported.
///
/// Like asynchronous channels, if the [`Receiver`] is disconnected while trying
/// to [`send`] with the [`SyncSender`], the [`send`] method will return a
/// [`SendError`]. Similarly, If the [`SyncSender`] is disconnected while trying
/// to [`recv`], the [`recv`] method will return a [`RecvError`].
///
/// See the [standard library] for further details.
///
/// [`send`]: SyncSender::send
/// [`recv`]: Receiver::recv
/// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/fn.sync_channel.html
#[must_use]
pub fn sync_channel<T>(bound: usize) -> (SyncSender<T>, Receiver<T>) {
    debug_assert!(bound > 0, "sync_channel bound must be non-zero"); // ArrayQueue::new will check again
    let channal = Arc::new(ArrayChannel::<T>::new(bound));
    let cloned = channal.clone();
    (
        SyncSender { inner: channal },
        Receiver {
            inner: InnerReceiver::Array(cloned),
            _marker: PhantomData,
        },
    )
}

/// An iterator over messages on a [`Receiver`], created by [`iter`].
///
/// This iterator will block whenever [`next`] is called,
/// waiting for a new message, and [`None`] will be returned
/// when the corresponding channel has hung up.
///
/// [`iter`]: Receiver::iter
/// [`next`]: Iterator::next
#[derive(Debug)]
pub struct Iter<'a, T: 'a> {
    rx: &'a Receiver<T>,
}

/// An iterator that attempts to yield all pending values for a [`Receiver`],
/// created by [`try_iter`].
///
/// [`None`] will be returned when there are no pending values remaining or
/// if the corresponding channel has hung up.
///
/// This iterator will never block the caller in order to wait for data to
/// become available. Instead, it will return [`None`].
///
/// [`try_iter`]: Receiver::try_iter
#[derive(Debug)]
pub struct TryIter<'a, T: 'a> {
    rx: &'a Receiver<T>,
}

/// An owning iterator over messages on a [`Receiver`],
/// created by [`into_iter`].
///
/// This iterator will block whenever [`next`]
/// is called, waiting for a new message, and [`None`] will be
/// returned if the corresponding channel has hung up.
///
/// [`into_iter`]: Receiver::into_iter
/// [`next`]: Iterator::next
#[derive(Debug)]
pub struct IntoIter<T> {
    rx: Receiver<T>,
}

////////////////////////////////////////////////////////////////////////////////
// Sender
////////////////////////////////////////////////////////////////////////////////

impl<T> Sender<T> {
    /// Attempts to send a value on this channel, returning it back if it could
    /// not be sent.
    ///
    /// A successful send occurs when it is determined that the other end of
    /// the channel has not hung up already. An unsuccessful send would be one
    /// where the corresponding receiver has already been deallocated. Note
    /// that a return value of [`Err`] means that the data will never be
    /// received, but a return value of [`Ok`] does *not* mean that the data
    /// will be received. It is possible for the corresponding receiver to
    /// hang up immediately after this function returns [`Ok`].
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/struct.Sender.html#method.send
    #[inline]
    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        self.inner.send(t)
    }
}

impl<T> Clone for Sender<T> {
    /// Clone a sender to send to other threads.
    ///
    /// Note, be aware of the lifetime of the sender because all senders
    /// (including the original) need to be dropped in order for
    /// [`Receiver::recv`] to stop blocking.
    fn clone(&self) -> Sender<T> {
        self.inner.add_senders();
        Sender {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.inner.sub_senders();
    }
}

impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sender").finish_non_exhaustive()
    }
}

////////////////////////////////////////////////////////////////////////////////
// SyncSender
////////////////////////////////////////////////////////////////////////////////

impl<T> SyncSender<T> {
    /// Sends a value on this synchronous channel.
    ///
    /// This function will *block* until space in the internal buffer becomes
    /// available or a receiver is available to hand off the message to.
    ///
    /// Note that a successful send does *not* guarantee that the receiver will
    /// ever see the data if there is a buffer on this channel. Items may be
    /// enqueued in the internal buffer for the receiver to receive at a later
    /// time. If the buffer size is 0, however, the channel becomes a rendezvous
    /// channel and it guarantees that the receiver has indeed received
    /// the data if this function returns success.
    ///
    /// This function will never panic, but it may return [`Err`] if the
    /// [`Receiver`] has disconnected and is no longer able to receive
    /// information.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/struct.SyncSender.html#method.send
    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        self.inner.send(t)
    }

    /// Attempts to send a value on this channel without blocking.
    ///
    /// This method differs from [`send`] by returning immediately if the
    /// channel's buffer is full or no receiver is waiting to acquire some
    /// data. Compared with [`send`], this function has two failure cases
    /// instead of one (one for disconnection, one for a full buffer).
    ///
    /// See [`send`] for notes about guarantees of whether the
    /// receiver has received the data or not if this function is successful.
    ///
    /// See the [standard library] for further details.
    ///
    /// [`send`]: Self::send
    /// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/struct.SyncSender.html#method.try_send
    pub fn try_send(&self, t: T) -> Result<(), TrySendError<T>> {
        self.inner.try_send(t)
    }
}

impl<T> Clone for SyncSender<T> {
    fn clone(&self) -> SyncSender<T> {
        self.inner.add_senders();
        SyncSender {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Drop for SyncSender<T> {
    fn drop(&mut self) {
        self.inner.sub_senders();
    }
}

impl<T> fmt::Debug for SyncSender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncSender").finish_non_exhaustive()
    }
}

////////////////////////////////////////////////////////////////////////////////
// Receiver
////////////////////////////////////////////////////////////////////////////////

impl<T> Receiver<T> {
    /// Attempts to return a pending value on this receiver without blocking.
    ///
    /// This method will never block the caller in order to wait for data to
    /// become available. Instead, this will always return immediately with a
    /// possible option of pending data on the channel.
    ///
    /// This is useful for a flavor of "optimistic check" before deciding to
    /// block on a receiver.
    ///
    /// Compared with [`recv`], this function has two failure cases instead of one
    /// (one for disconnection, one for an empty buffer).
    ///
    /// See the [standard library] for further details.
    ///
    /// [`recv`]: Self::recv
    /// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html#method.try_recv
    #[inline]
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        match &self.inner {
            InnerReceiver::List(channel) => channel.try_recv(),
            InnerReceiver::Array(channel) => channel.try_recv(),
        }
    }

    /// Attempts to wait for a value on this receiver, returning an error if the
    /// corresponding channel has hung up.
    ///
    /// This function will always block the current thread if there is no data
    /// available and it's possible for more data to be sent (at least one sender
    /// still exists). Once a message is sent to the corresponding [`Sender`]
    /// (or [`SyncSender`]), this receiver will wake up and return that
    /// message.
    ///
    /// If the corresponding [`Sender`] has disconnected, or it disconnects while
    /// this call is blocking, this call will wake up and return [`Err`] to
    /// indicate that no more messages can ever be received on this channel.
    /// However, since channels are buffered, messages sent before the disconnect
    /// will still be properly received.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html#method.recv
    #[inline]
    pub fn recv(&self) -> Result<T, RecvError> {
        match &self.inner {
            InnerReceiver::List(channel) => channel.recv(),
            InnerReceiver::Array(channel) => channel.recv(),
        }
    }

    /// Attempts to wait for a value on this receiver, returning an error if the
    /// corresponding channel has hung up, or if it waits more than `timeout`.
    ///
    /// This function will always block the current thread if there is no data
    /// available and it's possible for more data to be sent (at least one sender
    /// still exists). Once a message is sent to the corresponding [`Sender`]
    /// (or [`SyncSender`]), this receiver will wake up and return that
    /// message.
    ///
    /// If the corresponding [`Sender`] has disconnected, or it disconnects while
    /// this call is blocking, this call will wake up and return [`Err`] to
    /// indicate that no more messages can ever be received on this channel.
    /// However, since channels are buffered, messages sent before the disconnect
    /// will still be properly received.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html#method.recv_timeout
    #[inline]
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        match &self.inner {
            InnerReceiver::List(channel) => channel.recv_timeout(timeout),
            InnerReceiver::Array(channel) => channel.recv_timeout(timeout),
        }
    }

    /// Returns an iterator that will block waiting for messages, but never
    /// [`panic!`]. It will return [`None`] when the channel has hung up.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html#method.iter
    #[inline(always)]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter { rx: self }
    }

    /// Returns an iterator that will attempt to yield all pending values.
    /// It will return `None` if there are no more pending values or if the
    /// channel has hung up. The iterator will never [`panic!`] or block the
    /// user by waiting for values.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html#method.try_iter
    #[inline(always)]
    pub fn try_iter(&self) -> TryIter<'_, T> {
        TryIter { rx: self }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        match &mut self.inner {
            InnerReceiver::List(channel) => channel.sub_receivers(),
            InnerReceiver::Array(channel) => channel.sub_receivers(),
        }
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = T;
    #[inline]
    fn next(&mut self) -> Option<T> {
        self.rx.recv().ok()
    }
}

impl<'a, T> Iterator for TryIter<'a, T> {
    type Item = T;
    #[inline]
    fn next(&mut self) -> Option<T> {
        self.rx.try_recv().ok()
    }
}

impl<'a, T> IntoIterator for &'a Receiver<T> {
    type Item = T;
    type IntoIter = Iter<'a, T>;
    #[inline]
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;
    #[inline]
    fn next(&mut self) -> Option<T> {
        self.rx.recv().ok()
    }
}

impl<T> IntoIterator for Receiver<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    #[inline(always)]
    fn into_iter(self) -> IntoIter<T> {
        IntoIter { rx: self }
    }
}

impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Receiver").finish_non_exhaustive()
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use alloc::vec::Vec;
    use alloc::vec;
    use core::time::Duration;
    use std::thread;

    use super::{RecvTimeoutError, TryRecvError, TrySendError, channel, sync_channel};

    #[test]
    fn channel_send_recv_order_and_hangup() {
        let (tx, rx) = channel();
        let tx2 = tx.clone();

        tx.send(1).unwrap();
        tx2.send(2).unwrap();

        drop(tx);
        drop(tx2);

        assert_eq!(rx.recv().unwrap(), 1);
        assert_eq!(rx.recv().unwrap(), 2);
        // now channel is closed
        assert!(rx.recv().is_err());
    }

    #[test]
    fn sender_clone_disconnects_only_after_all_dropped() {
        let (tx, rx) = channel();
        let tx2 = tx.clone();
        tx.send(10).unwrap();
        drop(tx);
        assert_eq!(rx.recv().unwrap(), 10);
        drop(tx2);
        assert!(rx.recv().is_err());
    }

    #[test]
    fn try_recv_and_try_iter_behaviour() {
        let (tx, rx) = channel();
        // empty -> WouldBlock/Empty
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));

        tx.send(3).unwrap();
        tx.send(4).unwrap();

        let mut got = Vec::new();
        for v in rx.try_iter() {
            got.push(v);
        }
        assert_eq!(got, [3, 4]);
    }

    #[test]
    fn sync_channel_try_send_full() {
        let (tx, rx) = sync_channel(1);
        // first fits
        tx.try_send(1).unwrap();
        // buffer full
        assert!(matches!(tx.try_send(2), Err(TrySendError::Full(_))));

        // receiver can take one, then try_send should succeed
        assert_eq!(rx.recv().unwrap(), 1);
        tx.try_send(2).unwrap();
        assert_eq!(rx.recv().unwrap(), 2);
    }

    #[test]
    fn recv_timeout_behaviour() {
        let (_tx, rx) = channel::<i32>();
        let res = rx.recv_timeout(Duration::from_millis(20));
        assert!(matches!(res, Err(RecvTimeoutError::Timeout)));
    }

    #[test]
    fn into_iter_consumes_and_stops_on_hangup() {
        let (tx, rx) = channel();
        tx.send(7).unwrap();
        drop(tx);
        let v: Vec<_> = rx.into_iter().collect();
        assert_eq!(v, vec![7]);
    }

    #[test]
    fn iter_blocks_until_hangup_and_yields_all() {
        let (tx, rx) = channel();
        let handle = thread::spawn(move || {
            let mut collected = Vec::new();
            for v in rx.iter() {
                collected.push(v);
            }
            collected
        });

        // send a few messages then drop sender
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        tx.send(3).unwrap();
        drop(tx);

        let res = handle.join().unwrap();
        assert_eq!(res, vec![1, 2, 3]);
    }
}
