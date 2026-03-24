//! Low-level synchronization primitives and concurrent data structures.
//!
//! This module contains building blocks implemented around atomics and lock-free or spin-based
//! techniques. They are intended for runtime internals and performance-sensitive paths.
//!
//! # Synchronization Primitives
//!
//! - [`OnceFlag`]: A lightweight one-time state flag.
//! - [`Futex`]: A minimal spin-based synchronization primitive.
//! - [`SpinLock`]: A mutex-like lock where waiters spin instead of parking.
//! - [`SpinLockGuard`]: RAII guard returned by [`SpinLock`].
//!
//! # Queue Structures
//!
//! - [`ArrayQueue`]: A bounded MPMC queue backed by a fixed-size ring buffer.
//! - [`ListQueue`]: An unbounded queue backed by linked blocks with idle-block reuse.
//! - [`Parallel`]: A per-thread local aggregator for collecting and draining thread-local values.
//!
//! # Execution Helpers
//!
//! - [`CachePadded`]: Cache-line padding wrapper to reduce false sharing.
//! - [`Backoff`]: Backoff strategy utility for contention-heavy retry loops.

// -----------------------------------------------------------------------------
// Modules

mod array_queue;
mod backoff;
mod cache_paded;
mod futex;
mod list_queue;
mod once_flag;
mod parallel;
mod spin_lock;

// -----------------------------------------------------------------------------
// Exports

pub use array_queue::ArrayQueue;
pub use backoff::Backoff;
pub use cache_paded::CachePadded;
pub use futex::Futex;
pub use list_queue::ListQueue;
pub use once_flag::OnceFlag;
pub use parallel::Parallel;
pub use spin_lock::{SpinLock, SpinLockGuard};

// -----------------------------------------------------------------------------
// Utils for test

#[cfg(all(test, feature = "std"))]
#[allow(dead_code, reason = "tests")]
pub(crate) mod tests {
    use core::{any::Any, panic::AssertUnwindSafe, sync::atomic};
    use std::{boxed::Box, panic, thread};

    pub(crate) fn test_unwind_panic<R>(f: impl FnOnce() -> R) -> Result<R, Box<dyn Any + Send>> {
        let prev_hook = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));

        let result = panic::catch_unwind(AssertUnwindSafe(f));

        panic::set_hook(prev_hook);
        result
    }

    pub(crate) fn test_thread_panic<F, T>(f: F) -> Result<T, Box<dyn Any + Send>>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        let prev_hook = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));
        atomic::fence(atomic::Ordering::SeqCst);
        let result = thread::spawn(f).join();
        panic::set_hook(prev_hook);
        result
    }
}
