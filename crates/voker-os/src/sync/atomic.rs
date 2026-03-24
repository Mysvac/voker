//! Provide atomic types
//!
//! If the target platform does not have a corresponding atomic type,
//! this will switch to `portable_atomic`.
//!
//! This does not include atomic pointers, which are currently required.
//!
//! See the [standard library] for further details.
//!
//! [standard library]: https://doc.rust-lang.org/core/sync/atomic

pub use atomic_8::{AtomicBool, AtomicI8, AtomicU8};
pub use atomic_16::{AtomicI16, AtomicU16};
pub use atomic_32::{AtomicI32, AtomicU32};
pub use atomic_64::{AtomicI64, AtomicU64};
pub use core::sync::atomic::{AtomicIsize, AtomicPtr, AtomicUsize};
pub use core::sync::atomic::{Ordering, compiler_fence, fence};

#[cfg(target_has_atomic = "8")]
use core::sync::atomic as atomic_8;

#[cfg(not(target_has_atomic = "8"))]
use portable_atomic as atomic_8;

#[cfg(target_has_atomic = "16")]
use core::sync::atomic as atomic_16;

#[cfg(not(target_has_atomic = "16"))]
use portable_atomic as atomic_16;

#[cfg(target_has_atomic = "32")]
use core::sync::atomic as atomic_32;

#[cfg(not(target_has_atomic = "32"))]
use portable_atomic as atomic_32;

#[cfg(target_has_atomic = "64")]
use core::sync::atomic as atomic_64;

#[cfg(not(target_has_atomic = "64"))]
use portable_atomic as atomic_64;

#[cfg(not(target_has_atomic = "ptr"))]
compile_error!("Platforms without atomic pointers are currently not supported.");
