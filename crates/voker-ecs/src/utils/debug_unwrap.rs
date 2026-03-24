/// A debug checked version of [`Option::unwrap_unchecked`].
///
/// Will panic in debug modes if unwrapping a `None` or `Err` value in debug mode,
/// but is equivalent to `Option::unwrap_unchecked` or `Result::unwrap_unchecked`
/// in release mode.
#[doc(hidden)]
pub trait DebugCheckedUnwrap {
    type Item;

    /// # Safety
    /// This must never be called on a `None` or `Err` value. This can
    /// only be called on `Some` or `Ok` values.
    unsafe fn debug_checked_unwrap(self) -> Self::Item;
}

impl<T> DebugCheckedUnwrap for Option<T> {
    type Item = T;

    crate::cfg::debug! {
        if {
            #[inline(always)]
            #[track_caller]
            unsafe fn debug_checked_unwrap(self) -> Self::Item {
                if let Some(inner) = self {
                    inner
                } else {
                    unreachable!()
                }
            }
        } else {
            #[inline(always)]
            unsafe fn debug_checked_unwrap(self) -> Self::Item {
                unsafe { self.unwrap_unchecked() }
            }
        }
    }
}

impl<T, U> DebugCheckedUnwrap for Result<T, U> {
    type Item = T;

    crate::cfg::debug! {
        if {
            #[inline(always)]
            #[track_caller]
            unsafe fn debug_checked_unwrap(self) -> Self::Item {
                if let Ok(inner) = self {
                    inner
                } else {
                    unreachable!()
                }
            }
        } else {
            #[inline(always)]
            unsafe fn debug_checked_unwrap(self) -> Self::Item {
                unsafe { self.unwrap_unchecked() }
            }
        }
    }
}
