#![expect(unsafe_code, reason = "LazyLock requires unsafe code.")]

use core::cell::UnsafeCell;
use core::fmt;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::panic::{RefUnwindSafe, UnwindSafe};

use super::{Once, once::OnceExclusiveState};

// We use the state of a Once as discriminant value. Upon creation, the state is
// "incomplete" and `f` contains the initialization closure. In the first call to
// `call_once`, `f` is taken and run. If it succeeds, `value` is set and the state
// is changed to "complete". If it panics, the Once is poisoned, so none of the
// two fields is initialized.
union Data<T, F> {
    value: ManuallyDrop<T>,
    f: ManuallyDrop<F>,
}

/// Fallback implementation of `LazyLock` from the standard library.
///
/// A value which is initialized on the first access.
///
/// This type is a thread-safe `LazyCell`, and can be used in statics.
/// Since initialization may be called from multiple threads, any
/// dereferencing call will block the calling thread if another
/// initialization routine is currently running.
///
/// See the [standard library] for further details.
///
/// [standard library]: https://doc.rust-lang.org/std/sync/struct.LazyLock.html
pub struct LazyLock<T, F = fn() -> T> {
    // FIXME(nonpoison_once): if possible, switch to nonpoison version once it is available
    once: Once,
    data: UnsafeCell<Data<T, F>>,
}

impl<T, F: FnOnce() -> T> LazyLock<T, F> {
    /// Creates a new lazy value with the given initializing function.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_os::sync::LazyLock;
    ///
    /// let hello = "Hello, World!".to_string();
    ///
    /// let lazy = LazyLock::new(|| hello.to_uppercase());
    ///
    /// assert_eq!(&*lazy, "HELLO, WORLD!");
    /// ```
    #[inline]
    pub const fn new(f: F) -> LazyLock<T, F> {
        LazyLock {
            once: Once::new(),
            data: UnsafeCell::new(Data {
                f: ManuallyDrop::new(f),
            }),
        }
    }

    /// Forces the evaluation of this lazy value and returns a reference to
    /// result. This is equivalent to the `Deref` impl, but is explicit.
    ///
    /// This method will block the calling thread if another initialization
    /// routine is currently running.
    ///
    /// # Panics
    ///
    /// If the initialization closure panics (the one that is passed to the [`new()`] method), the
    /// panic is propagated to the caller, and the lock becomes poisoned. This will cause all future
    /// accesses of the lock (via [`force()`] or a dereference) to panic.
    ///
    /// [`new()`]: LazyLock::new
    /// [`force()`]: LazyLock::force
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_os::sync::LazyLock;
    ///
    /// let lazy = LazyLock::new(|| 92);
    ///
    /// assert_eq!(LazyLock::force(&lazy), &92);
    /// assert_eq!(&*lazy, &92);
    /// ```
    #[inline]
    pub fn force(this: &LazyLock<T, F>) -> &T {
        this.once.call_once_force(|state| {
            if state.is_poisoned() {
                panic_poisoned();
            }

            // SAFETY: `call_once` only runs this closure once, ever.
            let data = unsafe { &mut *this.data.get() };
            let f = unsafe { ManuallyDrop::take(&mut data.f) };
            let value = f();
            data.value = ManuallyDrop::new(value);
        });

        unsafe { &(*this.data.get()).value }
    }

    /// Forces the evaluation of this lazy value and returns a mutable reference to
    /// the result.
    ///
    /// # Panics
    ///
    /// If the initialization closure panics (the one that is passed to the [`new()`] method), the
    /// panic is propagated to the caller, and the lock becomes poisoned. This will cause all future
    /// accesses of the lock (via [`force()`] or a dereference) to panic.
    ///
    /// [`new()`]: LazyLock::new
    /// [`force()`]: LazyLock::force
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_os::sync::LazyLock;
    ///
    /// let mut lazy = LazyLock::new(|| 92);
    ///
    /// let p = LazyLock::force_mut(&mut lazy);
    /// assert_eq!(*p, 92);
    /// *p = 44;
    /// assert_eq!(*lazy, 44);
    /// ```
    #[inline]
    pub fn force_mut(this: &mut LazyLock<T, F>) -> &mut T {
        this.once.call_once_force(|state| {
            if state.is_poisoned() {
                panic_poisoned();
            }

            // SAFETY: `call_once` only runs this closure once, ever.
            let data = unsafe { &mut *this.data.get() };
            let f = unsafe { ManuallyDrop::take(&mut data.f) };
            let value = f();
            data.value = ManuallyDrop::new(value);
        });

        unsafe { &mut (*this.data.get()).value }
    }
}

impl<T, F> LazyLock<T, F> {
    /// Returns a mutable reference to the value if initialized.
    /// Otherwise (if uninitialized or poisoned), returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_os::sync::LazyLock;
    ///
    /// let mut lazy = LazyLock::new(|| 92);
    ///
    /// assert_eq!(LazyLock::get_mut(&mut lazy), None);
    /// let _ = LazyLock::force(&lazy);
    /// *LazyLock::get_mut(&mut lazy).unwrap() = 44;
    /// assert_eq!(*lazy, 44);
    /// ```
    #[inline]
    pub fn get_mut(this: &mut LazyLock<T, F>) -> Option<&mut T> {
        // `state()` does not perform an atomic load, so prefer it over `is_complete()`.
        let state = this.once.exclusive_state();
        match state {
            // SAFETY:
            // The closure has been run successfully, so `value` has been initialized.
            OnceExclusiveState::Complete => Some(unsafe { &mut this.data.get_mut().value }),
            _ => None,
        }
    }

    /// Returns a reference to the value if initialized.
    /// Otherwise (if uninitialized or poisoned), returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_os::sync::LazyLock;
    ///
    /// let lazy = LazyLock::new(|| 92);
    ///
    /// assert_eq!(LazyLock::get(&lazy), None);
    /// let _ = LazyLock::force(&lazy);
    /// assert_eq!(LazyLock::get(&lazy), Some(&92));
    /// ```
    #[inline]
    pub fn get(this: &LazyLock<T, F>) -> Option<&T> {
        if this.once.is_completed() {
            // SAFETY:
            // The closure has been run successfully, so `value` has been initialized
            // and will not be modified again.
            Some(unsafe { &(*this.data.get()).value })
        } else {
            None
        }
    }
}

impl<T, F> Drop for LazyLock<T, F> {
    fn drop(&mut self) {
        match self.once.exclusive_state() {
            OnceExclusiveState::Incomplete => unsafe {
                ManuallyDrop::drop(&mut self.data.get_mut().f)
            },
            OnceExclusiveState::Complete => unsafe {
                ManuallyDrop::drop(&mut self.data.get_mut().value)
            },
            OnceExclusiveState::Poisoned => {}
        }
    }
}

impl<T, F: FnOnce() -> T> Deref for LazyLock<T, F> {
    type Target = T;
    /// Dereferences the value.
    ///
    /// This method will block the calling thread if another initialization
    /// routine is currently running.
    ///
    /// # Panics
    ///
    /// If the initialization closure panics (the one that is passed to the [`new()`] method), the
    /// panic is propagated to the caller, and the lock becomes poisoned. This will cause all future
    /// accesses of the lock (via [`force()`] or a dereference) to panic.
    ///
    /// [`new()`]: LazyLock::new
    /// [`force()`]: LazyLock::force
    #[inline]
    fn deref(&self) -> &T {
        LazyLock::force(self)
    }
}

impl<T, F: FnOnce() -> T> DerefMut for LazyLock<T, F> {
    /// # Panics
    ///
    /// If the initialization closure panics (the one that is passed to the [`new()`] method), the
    /// panic is propagated to the caller, and the lock becomes poisoned. This will cause all future
    /// accesses of the lock (via [`force()`] or a dereference) to panic.
    ///
    /// [`new()`]: LazyLock::new
    /// [`force()`]: LazyLock::force
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        LazyLock::force_mut(self)
    }
}

impl<T: Default> Default for LazyLock<T> {
    /// Creates a new lazy value using `Default` as the initializing function.
    #[inline]
    fn default() -> LazyLock<T> {
        LazyLock::new(T::default)
    }
}

impl<T: fmt::Debug, F> fmt::Debug for LazyLock<T, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_tuple("LazyLock");
        if self.once.is_completed() {
            d.field(unsafe { &(*self.data.get()).value });
        } else {
            d.field(&format_args!("<uninit>"));
        }

        d.finish()
    }
}

#[cold]
#[inline(never)]
fn panic_poisoned() -> ! {
    panic!("LazyLock instance has previously been poisoned")
}

unsafe impl<T: Sync + Send, F: Send> Sync for LazyLock<T, F> {}
impl<T: RefUnwindSafe + UnwindSafe, F: UnwindSafe> RefUnwindSafe for LazyLock<T, F> {}
impl<T: UnwindSafe, F: UnwindSafe> UnwindSafe for LazyLock<T, F> {}

#[cfg(all(test, feature = "std"))]
mod tests {
    use core::ops::DerefMut;
    use alloc::format;
    use alloc::string::String;
    use alloc::vec::Vec;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use core::time::Duration;

    use super::LazyLock;
    use crate::utils::tests::test_unwind_panic;

    #[test]
    fn force_and_deref_return_value() {
        let l = LazyLock::new(|| 92u32);
        assert_eq!(*LazyLock::force(&l), 92);
        assert_eq!(*l, 92);
    }

    #[test]
    fn deref_mut_and_default() {
        let mut l = LazyLock::new(|| String::from("abc"));
        let s = l.deref_mut();
        s.push_str("d");
        assert_eq!(&*l, "abcd");

        let d: LazyLock<i32> = LazyLock::default();
        assert_eq!(*d, 0);
    }

    #[test]
    fn debug_uninit_and_init() {
        let l = LazyLock::new(|| 7i32);
        let s = format!("{:?}", l);
        assert!(s.contains("<uninit>"));
        LazyLock::force(&l);
        let s2 = format!("{:?}", l);
        assert!(s2.contains("7"));
    }

    struct DropCounter(Arc<AtomicUsize>);
    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct ValueDrop(Arc<AtomicUsize>);
    impl Drop for ValueDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn closure_dropped_if_uninitialized() {
        let counter = Arc::new(AtomicUsize::new(0));
        let dc = DropCounter(counter.clone());
        let l = LazyLock::new(move || {
            // capture dc inside closure; when closure is dropped without running,
            // dc will be dropped and increment the counter
            let _keep = &dc;
            1i32
        });

        drop(l);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn value_dropped_if_initialized() {
        let counter = Arc::new(AtomicUsize::new(0));
        let cd = counter.clone();
        let l = LazyLock::new(move || ValueDrop(cd));
        let _v = LazyLock::force(&l);
        drop(l);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn poisoning_propagates() {
        let l = LazyLock::new(|| panic!("init fail"));
        
        let _ = test_unwind_panic(|| LazyLock::force(&l));
        let r = test_unwind_panic(|| LazyLock::force(&l));

        assert!(r.is_err());
    }

    #[test]
    fn concurrent_init_runs_once() {
        const N: usize = 8;
        let counter = Arc::new(AtomicUsize::new(0));
        let c2 = counter.clone();
        let l = Arc::new(LazyLock::new(move || {
            thread::sleep(Duration::from_millis(10));
            c2.fetch_add(1, Ordering::SeqCst);
            55u32
        }));

        let mut hs = Vec::new();
        for _ in 0..N {
            let l2 = l.clone();
            hs.push(thread::spawn(move || {
                let v = LazyLock::force(&l2);
                assert_eq!(*v, 55);
            }));
        }
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
