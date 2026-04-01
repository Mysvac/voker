//! Provides a cross-platform `sleep` function.
//!
//! - In `std` environments, it directly re-exports `std::thread::sleep`.
//! - In non-`std` environments, a spin-based fallback is used.

pub use thread_impl::sleep;

crate::cfg::switch! {
    crate::cfg::std => {
        use std::thread as thread_impl;
    }
    _ => {
        mod __fallback;
        use __fallback as thread_impl;
    }
}

// -----------------------------------------------------------------------------
// thread_hash

/// Returns a hash value of the current thread ID.
///
/// If `std` is not support, the function always return `1`.
///
/// This hash may have collisions, so it is only recommended for thread checking
/// rather than as a unique identifier.
///
/// Based on the standard library implementation, `ThreadId` is currently a
/// wrapper around `u64`, so a no-op hasher is used directly.
///
/// If this value is intended for use in hash tables, consider applying a
/// second hash function.
pub fn thread_hash() -> u64 {
    use core::hash::{Hash, Hasher};

    struct NoOpHasher(u64);
    impl Hasher for NoOpHasher {
        fn finish(&self) -> u64 {
            self.0
        }

        fn write_u64(&mut self, i: u64) {
            self.0 = i;
        }

        fn write(&mut self, bytes: &[u8]) {
            for &byte in bytes.iter().rev() {
                self.0 = self.0.rotate_left(8) ^ (byte as u64);
            }
        }
    }

    let mut hasher = NoOpHasher(0);
    // For eliminating clippy hint
    hasher.write_u64(1);

    crate::cfg::std! {
        std::thread::current().id().hash(&mut hasher);
    }

    hasher.finish()
}

// -----------------------------------------------------------------------------
// available_parallelism

use core::num::NonZeroUsize;

/// Returns an estimate of the default amount of parallelism a program should use.
///
/// Similar to [`std::thread::available_parallelism`], but in no_std
/// environments (or when the std call fails) this returns `1`.
pub fn available_parallelism() -> NonZeroUsize {
    crate::cfg::switch! {
        crate::cfg::std => {
            #[expect(unsafe_code, reason = "`1` is non-zero")]
            std::thread::available_parallelism()
                .unwrap_or(unsafe{ NonZeroUsize::new_unchecked(1) })
        }
        _ => {
            #[expect(unsafe_code, reason = "`1` is non-zero")]
            unsafe{ NonZeroUsize::new_unchecked(1) }
        }
    }
}
