//! Copyright (c) 2019 The Crossbeam Project Developers
//!
//! See <https://docs.rs/crate/crossbeam-queue/latest>
//!
//! - Version: 0.8.4
//! - Date: 2026/04/01

use core::cell::Cell;
use core::fmt;

/// The maximum exponent of spin count.
const SPIN_LIMIT: u32 = 5;

/// Performs exponential backoff in spin loops.
///
/// Backing off in spin loops reduces contention and
/// improves overall performance.
///
/// This primitive can execute *YIELD* and *PAUSE* instructions,
/// yield the current thread to the OS scheduler, and tell when
/// is a good time to block the thread using a different synchronization
/// mechanism. Each step of the back off procedure takes roughly
/// twice as long as the previous step.
#[repr(transparent)]
pub struct Backoff {
    step: Cell<u32>,
}

impl Backoff {
    /// Creates a new `Backoff`.
    #[inline(always)]
    pub const fn new() -> Self {
        Self { step: Cell::new(0) }
    }

    /// Backs off in a lock-free loop.
    ///
    /// This method should be used when we need to retry an operation because another thread made
    /// progress.
    ///
    /// The processor may yield using the *YIELD* or *PAUSE* instruction.
    #[inline(always)]
    pub fn spin(&self) {
        let step: u32 = 1 << self.step.get();
        for _ in 0..step {
            core::hint::spin_loop();
        }

        if self.step.get() < SPIN_LIMIT {
            self.step.update(|v| v + 1);
        }
    }

    /// Backs off in a blocking loop.
    ///
    /// This method should be used when we need to wait for another thread to make progress.
    ///
    /// The processor may yield using the *YIELD* or *PAUSE* instruction and the current thread
    /// may yield by giving up a timeslice to the OS scheduler.
    ///
    /// In `#[no_std]` environments, this method is equivalent to [`spin`].
    ///
    /// [`spin`]: Backoff::spin
    #[inline]
    pub fn snooze(&self) {
        if self.step.get() <= SPIN_LIMIT {
            let step: u32 = 1 << { self.step.get() << 1 };

            for _ in 0..step {
                core::hint::spin_loop();
            }

            self.step.update(|v| v + 1);
        } else {
            #[cfg(not(feature = "std"))]
            for _ in 0..1024_u32 {
                core::hint::spin_loop();
            }

            #[cfg(feature = "std")]
            ::std::thread::yield_now();
        }
    }
}

impl fmt::Debug for Backoff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Backoff").field("step", &self.step).finish()
    }
}

impl Default for Backoff {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}
