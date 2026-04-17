//! Time abstractions.
//!
//! This module provides a cross-platform alternative to the standard library's `time` module.
//! - In `std` environments, it directly re-exports the standard library's contents.
//! - In `no_std` environments, fallback implementations are used as needed
//!   (see the `fallback` module for details).
//!
//! We strive to ensure that fallback implementations maintain the same API as the standard library
//! (only stable APIs). Some newer APIs may not be immediately available;
//! please submit an Issue in the [repository](https://github.com/voker-Engine/vc-core) for such cases.
//!
//! See the [standard library](https://doc.rust-lang.org/std/time) for further details.

pub use core::time::{Duration, TryFromFloatSecsError};
pub use time_impl::{Instant, SystemTime, SystemTimeError};

crate::cfg::switch! {
    crate::cfg::std => {
        use ::std::time as time_impl;
    }
    _ => {
        mod __fallback;
        use __fallback as time_impl;
    }
}
