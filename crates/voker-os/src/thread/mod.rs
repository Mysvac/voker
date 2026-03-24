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
// available_parallelism

use core::num::NonZero;

/// Returns an estimate of the default amount of parallelism a program should use.
///
/// Similar to [`std::thread::available_parallelism`], but in no_std
/// environments (or when the std call fails) this returns `1`.
pub fn available_parallelism() -> NonZero<usize> {
    crate::cfg::switch! {
        crate::cfg::std => {
            #[expect(unsafe_code, reason = "`1` is non-zero")]
            std::thread::available_parallelism()
                .unwrap_or(unsafe{ NonZero::new_unchecked(1) })
        }
        _ => {
            #[expect(unsafe_code, reason = "`1` is non-zero")]
            unsafe{ NonZero::new_unchecked(1) }
        }
    }
}
