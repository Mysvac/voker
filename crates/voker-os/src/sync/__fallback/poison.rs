use core::{error::Error, fmt};

/// Fallback implementation of `PoisonError` from the standard library.
///
/// A type of error which can be returned whenever a lock is acquired.
///
/// Since spin-lock implementations are immune to thread panic,
/// synchronization primitives will never become poisoned.
/// Nevertheless, this wrapper is provided to maintain API compatibility with the standard library.
///
/// See the [standard library] for further details.
///
/// [standard library]: https://doc.rust-lang.org/std/sync/struct.PoisonError.html
pub struct PoisonError<T> {
    guard: T,
}

/// Fallback implementation of `TryLockError` from the standard library.
///
/// An enumeration of possible errors associated with a [`TryLockResult`] which
/// can occur while trying to acquire a lock, from the [`try_lock`] method on a
/// [`Mutex`] or the [`try_read`] and [`try_write`] methods on an [`RwLock`].
///
/// See the [standard library] for further details.
///
/// [`Mutex`]: super::Mutex
/// [`try_lock`]: super::Mutex::try_lock
/// [`RwLock`]: super::RwLock
/// [`try_write`]: super::RwLock::try_write
/// [`try_read`]: super::RwLock::try_read
/// [standard library]: https://doc.rust-lang.org/std/sync/enum.TryLockError.html
pub enum TryLockError<T> {
    /// The lock could not be acquired because another thread failed while holding
    /// the lock.
    Poisoned(PoisonError<T>),
    /// The lock could not be acquired at this time because the operation would
    /// otherwise block.
    WouldBlock,
}

/// Fallback implementation of `LockResult` from the standard library.
///
/// A type alias for the result of a lock method which can be poisoned.
///
/// The [`Ok`] variant of this result indicates that the primitive was not
/// poisoned, and the operation result is contained within. The [`Err`] variant indicates
/// that the primitive was poisoned. Note that the [`Err`] variant *also* carries
/// an associated value assigned by the lock method, and it can be acquired through the
/// [`into_inner`] method. The semantics of the associated value depends on the corresponding
/// lock method.
///
/// See the [standard library] for further details.
///
/// [`into_inner`]: super::Mutex::into_inner
/// [standard library]: https://doc.rust-lang.org/std/sync/type.LockResult.html
pub type LockResult<Guard> = Result<Guard, PoisonError<Guard>>;

/// Fallback implementation of `TryLockResult` from the standard library.
///
/// A type alias for the result of a nonblocking locking method.
///
/// For more information, see [`LockResult`]. A `TryLockResult` doesn't
/// necessarily hold the associated guard in the [`Err`] type as the lock might not
/// have been acquired for other reasons.
///
/// See the [standard library] for further details.
///
/// [standard library]: https://doc.rust-lang.org/std/sync/type.TryLockResult.html
pub type TryLockResult<Guard> = Result<Guard, TryLockError<Guard>>;

impl<T> fmt::Debug for PoisonError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PoisonError").finish_non_exhaustive()
    }
}

impl<T> fmt::Display for PoisonError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "poisoned lock: another task failed inside".fmt(f)
    }
}

impl<T> Error for PoisonError<T> {}

impl<T> PoisonError<T> {
    /// Creates a `PoisonError`.
    ///
    /// See the [standard library](https://doc.rust-lang.org/std/sync) for further details.
    #[cfg(panic = "unwind")]
    pub fn new(guard: T) -> PoisonError<T> {
        PoisonError { guard }
    }

    /// Creates a `PoisonError`.
    ///
    /// This is generally created by methods like `Mutex::lock`
    /// or `RwLock::read`.
    ///
    /// This method may panic if std was built with `panic="abort"`.
    ///
    /// See the [standard library](https://doc.rust-lang.org/std/sync) for further details.
    #[cfg(not(panic = "unwind"))]
    #[track_caller]
    pub fn new(_data: T) -> PoisonError<T> {
        panic!("PoisonError created in a libstd built with panic=\"abort\"")
    }

    /// Consumes this error indicating that a lock is poisoned, returning the
    /// underlying guard to allow access regardless.
    ///
    /// See the [standard library](https://doc.rust-lang.org/std/sync) for further details.
    pub fn into_inner(self) -> T {
        self.guard
    }

    /// Reaches into this error indicating that a lock is poisoned, returning a
    /// reference to the underlying guard to allow access regardless.
    ///
    /// See the [standard library](https://doc.rust-lang.org/std/sync) for further details.
    pub fn get_ref(&self) -> &T {
        &self.guard
    }

    /// Reaches into this error indicating that a lock is poisoned, returning a
    /// mutable reference to the underlying guard to allow access regardless.
    ///
    /// See the [standard library](https://doc.rust-lang.org/std/sync) for further details.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

impl<T> From<PoisonError<T>> for TryLockError<T> {
    fn from(err: PoisonError<T>) -> TryLockError<T> {
        TryLockError::Poisoned(err)
    }
}

impl<T> fmt::Debug for TryLockError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TryLockError::Poisoned(..) => "Poisoned(..)".fmt(f),
            TryLockError::WouldBlock => "WouldBlock".fmt(f),
        }
    }
}

impl<T> fmt::Display for TryLockError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TryLockError::Poisoned(..) => "poisoned lock: another task failed inside",
            TryLockError::WouldBlock => "try_lock failed because the operation would block",
        }
        .fmt(f)
    }
}

impl<T> Error for TryLockError<T> {}
