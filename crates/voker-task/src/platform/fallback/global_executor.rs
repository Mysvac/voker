#![expect(unsafe_code, reason = "original implementation relies on unsafe")]

use core::panic::{RefUnwindSafe, UnwindSafe};

use super::LocalExecutor;

// -----------------------------------------------------------------------------
// SingleThreadExecutor

/// A wrapper for `LocalExecutor`, simulate thread_local.
///
/// The executor can only be run on the thread that created it.
#[repr(transparent)]
pub(super) struct GlobalExecutor<'a> {
    local: LocalExecutor<'a>,
}

impl<'a> GlobalExecutor<'a> {
    /// Creates a new executor.
    pub const fn new() -> Self {
        Self {
            local: LocalExecutor::new(),
        }
    }

    /// # Safety:
    ///
    /// Can only be called on the thread that created it.
    #[inline(always)]
    pub const unsafe fn inner(&self) -> &LocalExecutor<'a> {
        &self.local
    }
}

unsafe impl<'a> Send for GlobalExecutor<'a> {}
unsafe impl<'a> Sync for GlobalExecutor<'a> {}

impl UnwindSafe for GlobalExecutor<'_> {}
impl RefUnwindSafe for GlobalExecutor<'_> {}
