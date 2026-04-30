//! Provide fallback `std::sync` implementations based on spinlocks.
//!
//! Because synchronization is done through spinning, the primitives are
//! suitable for use in `no_std` environments.
//!
//! The API is intentionally kept compatible with the Rust standard library.
//!
//! If a standard library API becomes stable and this implementation has not yet
//! been updated, please submit an issue on GitHub.

// -----------------------------------------------------------------------------
// Modules

mod poison;
mod mutex;
mod rwlock;
mod lazylock;
mod once;

// -----------------------------------------------------------------------------
// Exports

pub use poison::{LockResult, PoisonError, TryLockError, TryLockResult};
pub use mutex::{Mutex, MutexGuard};
pub use rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
pub use once::{Once, OnceLock, OnceState};
pub use lazylock::LazyLock;
