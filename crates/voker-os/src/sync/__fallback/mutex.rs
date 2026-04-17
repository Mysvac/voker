#![expect(unsafe_code, reason = "Mutex requires unsafe code.")]

use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::panic::{RefUnwindSafe, UnwindSafe};

use super::{LockResult, TryLockError, TryLockResult};
use crate::utils::Futex;

/// Fallback implementation of `Mutex` from the standard library.
///
/// A mutual exclusion primitive useful for protecting shared data.
/// 
/// Implementation based on spin-locking, which will busy-wait (block)
/// the current thread until the lock is acquired.
///
/// Keep the API consistent with the [standard library].
///
/// # Poisoning
///
/// Although we provide interfaces consistent with the standard library,
/// this type's [`lock`] **will never fail** due to spin implementation.
///
/// Even if the thread panic, [`MutexGuard`] can release the lock during [`Drop`] normally.
///
/// See the [standard library] for further details.
/// 
/// [`lock`]: Mutex::lock
/// [standard library]: https://doc.rust-lang.org/std/sync/struct.Mutex.html
pub struct Mutex<T: ?Sized> {
    futex: Futex,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}
impl<T: ?Sized> UnwindSafe for Mutex<T> {}
impl<T: ?Sized> RefUnwindSafe for Mutex<T> {}

/// Fallback implementation of `MutexGuard` from the standard library.
///
/// An RAII implementation of a "scoped lock" of a mutex. When this structure is
/// dropped (falls out of scope), the lock will be unlocked.
///
/// The data protected by the mutex can be accessed through this guard via its
/// [`Deref`] and [`DerefMut`] implementations.
///
/// See the [standard library] for further details.
///
/// [standard library]: https://doc.rust-lang.org/std/sync/struct.MutexGuard.html
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a Mutex<T>,
    data: *mut T, // cache the pointer
}

// !Send is auto implemented
unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}
// UnwindSafe is auto implemented
impl<T: RefUnwindSafe + ?Sized> RefUnwindSafe for MutexGuard<'_, T> {}

impl<T> Mutex<T> {
    /// Creates a new mutex in an unlocked state ready for use.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_os::sync::Mutex;
    ///
    /// let mutex = Mutex::new(0);
    /// ```
    #[inline]
    pub const fn new(t: T) -> Self {
        Mutex {
            futex: Futex::new(),
            data: UnsafeCell::new(t),
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Acquires a mutex, blocking the current thread until it is able to do so.
    ///
    /// Due to spin implementation, this function always returns `Ok` .
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Mutex.html#method.lock
    #[inline]
    pub fn lock(&self) -> LockResult<MutexGuard<'_, T>> {
        self.futex.lock();
        Ok(MutexGuard {
            lock: self,
            data: self.data.get(),
        })
    }

    /// Attempts to acquire this lock.
    ///
    /// If the lock could not be acquired at this time, then [`Err`] is returned.
    /// Otherwise, an RAII guard is returned. The lock will be unlocked when the
    /// guard is dropped.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Mutex.html#method.try_lock
    #[inline]
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
        if self.futex.try_lock() {
            Ok(MutexGuard {
                lock: self,
                data: self.data.get(),
            })
        } else {
            Err(TryLockError::WouldBlock)
        }
    }

    /// Determines whether the mutex is poisoned.
    ///
    /// Due to spin implementation, this function always return `false`.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Mutex.html#method.is_poisoned
    #[inline(always)]
    pub fn is_poisoned(&self) -> bool {
        false
    }

    /// Clear the poisoned state from a mutex.
    ///
    /// Due to spin implementation, this function is no-op.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Mutex.html#method.clear_poison
    #[inline(always)]
    pub fn clear_poison(&self) {
        // no-op
    }

    /// Consumes this mutex, returning the underlying data.
    ///
    /// Due to spin implementation, this function always return `Ok`.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_os::sync::Mutex;
    ///
    /// let mutex = Mutex::new(0);
    /// assert_eq!(mutex.into_inner().unwrap(), 0);
    /// ```
    #[inline]
    pub fn into_inner(self) -> LockResult<T>
    where
        T: Sized,
    {
        Ok(self.data.into_inner())
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Due to spin implementation, this function always return `Ok`.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_os::sync::Mutex;
    ///
    /// let mut mutex = Mutex::new(0);
    /// *mutex.get_mut().unwrap() = 10;
    /// assert_eq!(*mutex.lock().unwrap(), 10);
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        Ok(unsafe { &mut *self.data.get() })
    }
}

impl<T> From<T> for Mutex<T> {
    /// Creates a new mutex in an unlocked state ready for use.
    /// This is equivalent to [`Mutex::new`].
    #[inline]
    fn from(t: T) -> Self {
        Mutex::new(t)
    }
}

impl<T: Default> Default for Mutex<T> {
    /// Creates a `Mutex<T>`, with the `Default` value for T.
    #[inline]
    fn default() -> Mutex<T> {
        Mutex::new(Default::default())
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("Mutex");
        match self.try_lock() {
            Ok(guard) => {
                d.field("data", &&*guard);
            }
            Err(_) => {
                // `TryLockError::Poisoned` is impossible
                d.field("data", &format_args!("<locked>"));
            }
        }
        d.field("poisoned", &false);
        d.finish_non_exhaustive()
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        unsafe { &*self.data }
    }
}

impl<'a, T: ?Sized> DerefMut for MutexGuard<'a, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data }
    }
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
    /// The dropping of the MutexGuard will release the lock it was created from.
    #[inline]
    fn drop(&mut self) {
        self.lock.futex.unlock();
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use std::fmt::Debug;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, mpsc::channel};
    use std::{hint, mem, thread};

    use super::Mutex;

    #[test]
    fn smoke() {
        let m = Mutex::new(());
        drop(m.lock().unwrap());
        drop(m.lock().unwrap());
    }

    #[test]
    fn lots_and_lots() {
        const J: u32 = 1000;
        const K: u32 = 3;

        let m = Arc::new(Mutex::new(0));

        fn inc(m: &Mutex<u32>) {
            for _ in 0..J {
                *m.lock().unwrap() += 1;
            }
        }

        let (tx, rx) = channel();
        for _ in 0..K {
            let tx2 = tx.clone();
            let m2 = m.clone();
            thread::spawn(move || {
                inc(&m2);
                tx2.send(()).unwrap();
            });
            let tx2 = tx.clone();
            let m2 = m.clone();
            thread::spawn(move || {
                inc(&m2);
                tx2.send(()).unwrap();
            });
        }

        drop(tx);
        for _ in 0..2 * K {
            rx.recv().unwrap();
        }
        assert_eq!(*m.lock().unwrap(), J * K * 2);
    }

    #[test]
    fn try_lock() {
        let m = Mutex::new(());
        *m.try_lock().unwrap() = ();
    }

    #[derive(Eq, PartialEq, Debug)]
    struct NonCopy(i32);

    #[derive(Eq, PartialEq, Debug)]
    struct NonCopyNeedsDrop(i32);

    impl Drop for NonCopyNeedsDrop {
        fn drop(&mut self) {
            hint::black_box(());
        }
    }

    #[test]
    fn test_needs_drop() {
        assert!(!mem::needs_drop::<NonCopy>());
        assert!(mem::needs_drop::<NonCopyNeedsDrop>());
    }

    #[test]
    fn test_into_inner() {
        let m = Mutex::new(NonCopy(10));
        assert_eq!(m.into_inner().unwrap(), NonCopy(10));
    }

    #[test]
    fn test_into_inner_drop() {
        struct Foo(Arc<AtomicUsize>);
        impl Drop for Foo {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let num_drops = Arc::new(AtomicUsize::new(0));
        let m = Mutex::new(Foo(num_drops.clone()));
        assert_eq!(num_drops.load(Ordering::SeqCst), 0);
        {
            let _inner = m.into_inner().unwrap();
            assert_eq!(num_drops.load(Ordering::SeqCst), 0);
        }
        assert_eq!(num_drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_get_mut() {
        let mut m = Mutex::new(NonCopy(10));
        *m.get_mut().unwrap() = NonCopy(20);
        assert_eq!(m.into_inner().unwrap(), NonCopy(20));
    }


    #[test]
    fn test_mutex_arc_nested() {
        // Tests nested mutexes and access
        // to underlying data.
        let arc = Arc::new(Mutex::new(1));
        let arc2 = Arc::new(Mutex::new(arc));
        let (tx, rx) = channel();
        let _t = thread::spawn(move || {
            let lock = arc2.lock().unwrap();
            let lock2 = lock.lock().unwrap();
            assert_eq!(*lock2, 1);
            tx.send(()).unwrap();
        });
        rx.recv().unwrap();
    }

    #[test]
    fn test_mutex_unsized() {
        let mutex: &Mutex<[i32]> = &Mutex::new([1, 2, 3]);
        {
            let b = &mut *mutex.lock().unwrap();
            b[0] = 4;
            b[2] = 5;
        }
        let comp: &[i32] = &[4, 2, 5];
        assert_eq!(&*mutex.lock().unwrap(), comp);
    }

    #[cfg(panic = "unwind")] // Requires unwinding support.
    #[test]
    fn test_panics() {
        use crate::utils::tests::test_unwind_panic;

        let mutex = Mutex::new(42);

        let result = test_unwind_panic(|| {
            let _guard1 = mutex.lock().unwrap();
            panic!("test panic with mutex once");
        });
        
        assert!(result.is_err());

        let result = test_unwind_panic(|| {
            let _guard2 = mutex.lock().unwrap();
            panic!("test panic with mutex twice");
        });
        assert!(result.is_err());

        let result = test_unwind_panic(|| {
            let _guard3 = mutex.lock().unwrap();
            panic!("test panic with mutex thrice");
        });
        assert!(result.is_err());
    }

    #[cfg(panic = "unwind")] // Requires unwinding support.
    #[test]
    fn test_mutex_arc_access_in_unwind() {
        use crate::utils::tests::test_thread_panic;

        let arc = Arc::new(Mutex::new(1));
        let arc2 = arc.clone();

        let _ = test_thread_panic(move || -> () {
            struct Unwinder {
                i: Arc<Mutex<i32>>,
            }
            impl Drop for Unwinder {
                fn drop(&mut self) {
                    *self.i.lock().unwrap() += 1;
                }
            }
            let _u = Unwinder { i: arc2 };
            panic!();
        });

        let lock = arc.lock().unwrap();
        assert_eq!(*lock, 2);
    }
}
