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

/// Defines a 32-bit id type which guarantees global uniqueness via atomics on a static global.
///
/// Note that this means the id space is process-wide, as such it may potentially be exhausted
/// by a combination of long-running processes and multiple `World`s, at which point we panic.
#[macro_export]
macro_rules! define_atomic_id {
    ($atomic_id_type:ident) => {
        /// Globally unique 32-bit id, guaranteed via atomics on a static global.
        ///
        /// Note that this means the id space is process-wide, as such it may potentially be exhausted
        /// by a combination of long-running processes and multiple `World`s, at which point we panic.
        #[derive(Copy, Clone, Hash, Eq, PartialEq, PartialOrd, Ord, Debug)]
        pub struct $atomic_id_type(::core::num::NonZeroU32);

        impl $atomic_id_type {
            /// Creates a new id via fetch_add atomic on a static global.
            #[inline]
            pub fn alloc() -> Self {
                #[cold]
                #[inline(never)]
                fn overflow() -> ! {
                    panic!("too many `{}`s.", stringify!($atomic_id_type));
                }

                use $crate::atomic::{AtomicU32, Ordering::Relaxed};

                static COUNTER: AtomicU32 = AtomicU32::new(1);

                let id = ALLOCATOR
                    .try_update(Relaxed, Relaxed, |val| val.checked_add(1))
                    .ok() // `1..u32::MAX`
                    .map(::core::num::NonZeroU32::new)
                    .unwrap_or_else(|| overflow());

                Self(id)
            }
        }

        impl From<$atomic_id_type> for ::core::num::NonZeroU32 {
            fn from(value: $atomic_id_type) -> Self {
                value.0
            }
        }

        impl From<::core::num::NonZeroU32> for $atomic_id_type {
            fn from(value: ::core::num::NonZeroU32) -> Self {
                Self(value)
            }
        }
    };
}
