use core::{error, fmt};

/// Fallback implementation of `SendError` from the standard library.
///
/// An error returned from the `Sender::send` or `SyncSender::send`
/// function on **channel**s.
///
/// See the [standard library] for further details.
///
/// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/struct.SendError.html
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct SendError<T>(pub T);

/// Fallback implementation of `RecvError` from the standard library.
///
/// An error returned from the `recv` function on a `Receiver`.
///
/// See the [standard library] for further details.
///
/// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/struct.RecvError.html
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct RecvError;

/// Fallback implementation of `TryRecvError` from the standard library.
///
/// This enumeration is the list of the possible reasons that `try_recv` could
/// not return data when called. This can occur with both a `channel` and
/// a `sync_channel`.
///
/// See the [standard library] for further details.
///
/// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/struct.SendError.html
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum TryRecvError {
    /// This **channel** is currently empty, but the **Sender**(s) have not yet
    /// disconnected, so data may yet become available.
    Empty,
    /// The **channel**'s sending half has become disconnected, and there will
    /// never be any more data received on it.
    Disconnected,
}

/// Fallback implementation of `RecvTimeoutError` from the standard library.
///
/// This enumeration is the list of possible errors that made `recv_timeout`
/// unable to return data when called. This can occur with both a `channel` and
/// a `sync_channel`.
///
/// See the [standard library] for further details.
///
/// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/enum.RecvTimeoutError.html
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum RecvTimeoutError {
    /// This **channel** is currently empty, but the **Sender**(s) have not yet
    /// disconnected, so data may yet become available.
    Timeout,
    /// The **channel**'s sending half has become disconnected, and there will
    /// never be any more data received on it.
    Disconnected,
}

/// Fallback implementation of `TrySendError` from the standard library.
///
/// This enumeration is the list of the possible error outcomes for the
/// `try_send` method.
///
/// See the [standard library] for further details.
///
/// [standard library]: https://doc.rust-lang.org/std/sync/mpsc/enum.TrySendError.html
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum TrySendError<T> {
    /// The data could not be sent on the `sync_channel` because it would require that
    /// the callee block to send the data.
    ///
    /// If this is a buffered channel, then the buffer is full at this time. If
    /// this is not a buffered channel, then there is no `Receiver` available to
    /// acquire the data.
    Full(T),
    /// This `sync_channel`'s receiving half has disconnected, so the data could not be
    /// sent. The data is returned back to the callee in this case.
    Disconnected(T),
}

impl<T> fmt::Debug for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendError").finish_non_exhaustive()
    }
}

impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "sending on a closed channel".fmt(f)
    }
}

impl<T> error::Error for SendError<T> {}

impl<T> fmt::Debug for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TrySendError::Full(..) => "Full(..)".fmt(f),
            TrySendError::Disconnected(..) => "Disconnected(..)".fmt(f),
        }
    }
}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TrySendError::Full(..) => "sending on a full channel".fmt(f),
            TrySendError::Disconnected(..) => "sending on a closed channel".fmt(f),
        }
    }
}

impl<T> error::Error for TrySendError<T> {}

impl<T> From<SendError<T>> for TrySendError<T> {
    /// Converts a `SendError<T>` into a `TrySendError<T>`.
    ///
    /// This conversion always returns a `TrySendError::Disconnected` containing the data in the `SendError<T>`.
    ///
    /// No data is allocated on the heap.
    fn from(err: SendError<T>) -> TrySendError<T> {
        match err {
            SendError(t) => TrySendError::Disconnected(t),
        }
    }
}

impl fmt::Display for RecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "receiving on a closed channel".fmt(f)
    }
}

impl error::Error for RecvError {}

impl fmt::Display for TryRecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TryRecvError::Empty => "receiving on an empty channel".fmt(f),
            TryRecvError::Disconnected => "receiving on a closed channel".fmt(f),
        }
    }
}

impl error::Error for TryRecvError {}

impl From<RecvError> for TryRecvError {
    /// Converts a `RecvError` into a `TryRecvError`.
    ///
    /// This conversion always returns `TryRecvError::Disconnected`.
    ///
    /// No data is allocated on the heap.
    fn from(err: RecvError) -> TryRecvError {
        match err {
            RecvError => TryRecvError::Disconnected,
        }
    }
}

impl fmt::Display for RecvTimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RecvTimeoutError::Timeout => "timed out waiting on channel".fmt(f),
            RecvTimeoutError::Disconnected => "channel is empty and sending half is closed".fmt(f),
        }
    }
}

impl error::Error for RecvTimeoutError {}

impl From<RecvError> for RecvTimeoutError {
    /// Converts a `RecvError` into a `RecvTimeoutError`.
    ///
    /// This conversion always returns `RecvTimeoutError::Disconnected`.
    ///
    /// No data is allocated on the heap.
    fn from(err: RecvError) -> RecvTimeoutError {
        match err {
            RecvError => RecvTimeoutError::Disconnected,
        }
    }
}
