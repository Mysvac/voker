//! Utility extensions and helpers for memory management.

use alloc::vec::Vec;
use core::ptr;

// -----------------------------------------------------------------------------
// AbortOnPanic

/// A guard that aborts the process when dropped during allocation failure.
///
/// # Why Abort?
///
/// In certain critical sections (like dropping or allocation), panicking could
/// lead to:
/// - Double panics (abort anyway)
/// - Resource leaks
/// - Unwinding across FFI boundaries
/// - Inconsistent internal state
///
/// By aborting immediately, we ensure the process terminates cleanly without
/// risking further corruption.
pub(super) struct AbortOnPanic;

impl Drop for AbortOnPanic {
    /// Aborts the process when dropped.
    ///
    /// This method is marked `#[cold]` and `#[inline(never)]` because:
    /// - It's an error path that rarely executes
    /// - We want to keep it out of hot code paths
    #[cold]
    #[inline(never)]
    fn drop(&mut self) {
        crate::cfg::std! {
            if {
                ::std::eprintln!("Aborting due to allocator error.");
                ::std::process::abort();
            } else {
                panic!("Aborting due to allocator error.");
            }
        }
    }
}

// -----------------------------------------------------------------------------
// VecRemoveExt

/// Efficient removal operations for [`Vec`] using swap-remove semantics.
///
/// This trait provides methods to remove elements from the end of a vector
/// without the overhead of bounds checking, and to move the last element to
/// an arbitrary position before removal. These operations are unsafe because
/// they require the caller to guarantee index validity.
pub(super) trait VecRemoveExt<T> {
    /// Removes and returns the last element without checking bounds.
    ///
    /// # Safety
    /// - `last_index` must be the index of the last element (vector length - 1)
    /// - The vector must have at least one element
    ///
    /// # Performance
    /// O(1), just reads the last element and updates length.
    unsafe fn remove_last(&mut self, last_index: usize) -> T;

    /// Moves the last element to a specified position and returns it.
    ///
    /// This combines a swap-remove operation into a single step:
    /// 1. Reads the last element
    /// 2. Writes it to the target position
    /// 3. Reduces the vector length
    /// 4. Return the copied **last** element.
    ///
    /// # Safety
    /// - `last_index` must be the index of the last element (vector length - 1)
    /// - `to` must be a valid index **less** than `last_index`
    /// - The vector must have at least one element
    ///
    /// # Performance
    /// O(1), just reads from end and writes to target position.
    unsafe fn move_last_to(&mut self, last_index: usize, to: usize) -> T;
}

impl<T: Copy> VecRemoveExt<T> for Vec<T> {
    #[inline(always)]
    unsafe fn remove_last(&mut self, last_index: usize) -> T {
        unsafe {
            let last = self.as_ptr().add(last_index);

            let value = ptr::read(last);
            self.set_len(last_index);
            value
        }
    }

    #[inline(always)]
    unsafe fn move_last_to(&mut self, last_index: usize, to: usize) -> T {
        let base_ptr = self.as_mut_ptr();

        unsafe {
            let src = base_ptr.add(last_index);
            let dst = base_ptr.add(to);

            let value = ptr::read(src);
            ptr::write(dst, value);
            self.set_len(last_index);
            value
        }
    }
}
