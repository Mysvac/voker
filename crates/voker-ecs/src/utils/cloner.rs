use voker_ptr::{OwningPtr, Ptr};

// -----------------------------------------------------------------------------
// Cloner

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Cloner {
    func: unsafe fn(Ptr<'_>, OwningPtr<'_>),
}

impl Cloner {
    /// # Safety
    /// - `src` and`dst` point to valid data.
    /// - the data of `dst` is uninit, so it's no need to drop.
    unsafe fn clone_via_clone<T: Clone>(src: Ptr<'_>, dst: OwningPtr<'_>) {
        src.debug_assert_aligned::<T>();
        dst.debug_assert_aligned::<T>();
        unsafe {
            let val = src.deref::<T>();
            let dst = dst.as_ptr() as *mut T;
            core::ptr::write::<T>(dst, val.clone());
        }
    }

    /// # Safety
    /// - `src` and`dst` point to valid data.
    unsafe fn clone_via_copy<T: Copy>(src: Ptr<'_>, dst: OwningPtr<'_>) {
        src.debug_assert_aligned::<T>();
        dst.debug_assert_aligned::<T>();
        unsafe {
            let src = src.as_ptr() as *const T;
            let dst = dst.as_ptr() as *mut T;
            core::ptr::copy_nonoverlapping::<T>(src, dst, 1);
        }
    }

    /// Creates a cloner that uses the [`Clone`] trait to duplicate the value.
    ///
    /// This is the standard cloning behavior for types that implement [`Clone`].
    /// It will call `clone()` on the source value and write the result to the destination.
    pub const fn clonable<T: Clone>() -> Self {
        Self {
            func: Self::clone_via_clone::<T>,
        }
    }

    /// Creates a cloner that uses a simple memory copy for [`Copy`] types.
    ///
    /// For types that implement [`Copy`], cloning is equivalent to a bitwise copy.
    /// This is more efficient than calling `clone()` as it bypasses the trait method
    /// and performs a direct memory copy.
    pub const fn copyable<T: Copy>() -> Self {
        Self {
            func: Self::clone_via_copy::<T>,
        }
    }
    /// # Safety
    /// Ensure by caller.
    #[inline(always)]
    pub unsafe fn call(self, src: Ptr<'_>, dst: OwningPtr<'_>) {
        unsafe {
            (self.func)(src, dst);
        }
    }
}
