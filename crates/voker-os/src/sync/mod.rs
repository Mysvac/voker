//! Useful synchronization primitives.
//!
//! This module provides a cross-platform alternative to the standard library's `sync` module.
//! - In `std` environments, it directly re-exports the standard library's contents.
//! - In non-`std` environments, different fallback implementations are used based on the situation.
//!
//! We strive to ensure that fallback implementations maintain the same API as the standard library
//! (only stable APIs). However, please note that while the API remains identical, internal implementations
//! may have some differences - for example, container sizes might differ from those in the standard library.
//!
//! Considering the update pace of the standard library, some newer APIs may not be immediately available;
//! please submit an Issue in the [repository](https://github.com/VoidCraft-Engine/vc-core) for such cases.
//!
//! See the [standard library] for further details.
//!
//! [standard library]: https://doc.rust-lang.org/std/sync/index.html
//!
//! ---
//!
//! ## atomic
//!
//! We detect whether atomic operations are available on the target platform.
//! If supported, we prioritize using `core::sync::atomic`; otherwise,
//! we fall back to `portable_atomic`.
//!
//! Note that the latter may expose additional APIs beyond the standard library,
//! and it is recommended to use only interfaces available in the standard library.
//!
//! Specifically, if the target platform does not support atomic pointers,
//! compilation will fail, as we rely on the standard library's `Arc`, which requires it.
//!
//! ## other
//!
//! When the `std` feature is enabled, we directly re-export the standard library's
//! APIs with zero additional overhead.
//!
//! If `std` is not supported, we fall back to spinlock-based implementations
//! while maintaining full API compatibility with the standard library.
//! (See `fallback` module for no_std support)

// -----------------------------------------------------------------------------
// Modules

mod sync_cell;
mod sync_unsafe_cell;

pub mod atomic;

// -----------------------------------------------------------------------------
// Exports

pub use alloc::sync::{Arc, Weak};
pub use sync_cell::SyncCell;
pub use sync_unsafe_cell::SyncUnsafeCell;

crate::cfg::switch! {
    crate::cfg::std => {
        pub use std::sync::{
            PoisonError, TryLockError, TryLockResult, LockResult,
            Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
            Barrier, BarrierWaitResult, Condvar, WaitTimeoutResult,
            Once, OnceLock, OnceState, LazyLock, mpsc,
        };
    }
    _ => {
        mod __fallback;
        pub use __fallback::{
            PoisonError, TryLockError, TryLockResult, LockResult,
            Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
            Barrier, BarrierWaitResult, Condvar, WaitTimeoutResult,
            Once, OnceLock, OnceState, LazyLock, mpsc,
        };
    }
}
