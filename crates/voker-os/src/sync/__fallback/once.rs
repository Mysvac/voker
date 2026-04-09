#![expect(unsafe_code, reason = "OnceLock requires unsafe code.")]

use core::fmt;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::cell::{Cell, UnsafeCell};
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::sync::atomic::Ordering::{Acquire, Relaxed, Release};

use crate::sync::atomic::AtomicU8;

/// Fallback implementation of `OnceState` from the standard library.
///
/// If the initialization function panics, the `Once` will become *poisoned*.
///
/// Keep the API consistent with the [standard library].
///
/// [standard library]: https://doc.rust-lang.org/std/sync/struct.OnceState.html
pub struct OnceState {
    poisoned: bool,
    set_state_to: Cell<u8>,
}

impl OnceState {
    /// Returns `true` if the associated [`Once`] was poisoned prior to the
    /// invocation of the closure passed to [`Once::call_once_force()`].
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.OnceState.html#method.is_poisoned
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }
}

impl fmt::Debug for OnceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OnceState")
            .field("poisoned", &self.is_poisoned())
            .finish()
    }
}

const POISONED: u8 = 3; // Function call paniced
const INCOMPLETE: u8 = 2;
const RUNNING: u8 = 1;
const COMPLETE: u8 = 0;

struct CompletionGuard<'a> {
    state: &'a AtomicU8,
    set_state_on_drop_to: u8,
}

impl<'a> Drop for CompletionGuard<'a> {
    fn drop(&mut self) {
        self.state.store(self.set_state_on_drop_to, Release);
    }
}

/// Fallback implementation of `Once` from the standard library.
///
/// A low-level synchronization primitive for one-time global execution.
///
/// If the initialization function panics, the `Once` will become *poisoned*.
///
/// Keep the API consistent with the [standard library].
///
/// [standard library]: https://doc.rust-lang.org/std/sync/struct.Once.html
pub struct Once {
    state: AtomicU8,
}

pub(crate) enum OnceExclusiveState {
    Complete,
    Incomplete,
    Poisoned,
}

impl Once {
    /// Creates a new `Once` value.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Once.html#method.new
    #[expect(
        clippy::new_without_default,
        reason = "`std::sync::Once` does not implement `Default`.")
    ]
    #[inline]
    #[must_use]
    pub const fn new() -> Once {
        Once {
            state: AtomicU8::new(INCOMPLETE),
        }
    }

    /// Returns `true` if some [`call_once()`] call has completed
    /// successfully. Specifically, `is_completed` will return false in
    /// the following situations:
    ///   * [`call_once()`] was not called at all,
    ///   * [`call_once()`] was called, but has not yet completed,
    ///   * the [`Once`] instance is poisoned
    ///
    /// This function returning `false` does not mean that [`Once`] has not been
    /// executed. For example, it may have been executed in the time between
    /// when `is_completed` starts executing and when it returns, in which case
    /// the `false` return value would be stale (but still permissible).
    ///
    /// See the [standard library] for further details.
    ///
    /// [`call_once()`]: Once::call_once
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Once.html#method.is_completed
    #[inline(always)]
    pub fn is_completed(&self) -> bool {
        // Use acquire ordering to make all initialization changes visible to the
        // current thread.
        self.state.load(Acquire) == COMPLETE
    }

    /// Performs an initialization routine once and only once. The given closure
    /// will be executed if this is the first time `call_once` has been called,
    /// and otherwise the routine will *not* be invoked.
    ///
    /// This method will block the calling thread if another initialization
    /// routine is currently running.
    ///
    /// When this function returns, it is guaranteed that some initialization
    /// has run and completed (it might not be the closure specified). It is also
    /// guaranteed that any memory writes performed by the executed closure can
    /// be reliably observed by other threads at this point (there is a
    /// happens-before relation between the closure and code executing after the
    /// return).
    ///
    /// # Panics
    ///
    /// The closure `f` will only be executed once even if this is called
    /// concurrently amongst many threads. If that closure panics, however, then
    /// it will *poison* this [`Once`] instance, causing all future invocations of
    /// `call_once` to also panic.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Once.html#method.is_completed
    #[inline]
    pub fn call_once<F>(&self, f: F)
    where
        F: FnOnce(),
    {
        // Fast path check
        if self.is_completed() {
            return;
        }

        let mut f = Some(f);
        self.call(false, &mut |_| f.take().unwrap()());
    }

    /// Performs the same function as [`call_once()`] except ignores poisoning.
    ///
    /// Unlike [`call_once()`], if this [`Once`] has been poisoned (i.e., a previous
    /// call to [`call_once()`] or [`call_once_force()`] caused a panic), calling
    /// [`call_once_force()`] will still invoke the closure `f` and will _not_
    /// result in an immediate panic. If `f` panics, the [`Once`] will remain
    /// in a poison state. If `f` does _not_ panic, the [`Once`] will no
    /// longer be in a poison state and all future calls to [`call_once()`] or
    /// [`call_once_force()`] will be no-ops.
    ///
    /// The closure `f` is yielded a [`OnceState`] structure which can be used
    /// to query the poison status of the [`Once`].
    ///
    /// [`call_once()`]: Once::call_once
    /// [`call_once_force()`]: Once::call_once_force
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Once.html#method.call_once_force
    #[inline]
    pub fn call_once_force<F>(&self, f: F)
    where
        F: FnOnce(&OnceState),
    {
        // Fast path check
        if self.is_completed() {
            return;
        }

        // Delay ownership transferring
        let mut f = Some(f);
        // the closure will only be moved when the take is executed.
        // Prior to this, if POISON caused a panic, the closure could be dropped here.
        self.call(true, &mut |p| f.take().unwrap()(p));
    }

    fn call(&self, ignore_poisoning: bool, f: &mut dyn FnMut(&OnceState)) {
        let backoff = crate::utils::Backoff::new();
        let mut state = self.state.load(Relaxed);
        loop {
            match state {
                COMPLETE => {
                    // Ensure visibility
                    core::sync::atomic::fence(Acquire);
                    return;
                }
                POISONED if !ignore_poisoning => {
                    panic!("Once instance has previously been poisoned");
                }
                INCOMPLETE | POISONED => {
                    if let Err(new) = self
                        .state
                        .compare_exchange_weak(state, RUNNING, Acquire, Relaxed)
                    {
                        state = new;
                        continue;
                    }

                    let f_state = OnceState {
                        poisoned: state == POISONED,
                        set_state_to: Cell::new(COMPLETE),
                    };

                    let mut completion_guard = CompletionGuard {
                        state: &self.state,
                        set_state_on_drop_to: POISONED,
                    };

                    f(&f_state);

                    completion_guard.set_state_on_drop_to = f_state.set_state_to.get();
                    return;
                }
                _ => {
                    assert!(state == RUNNING);
                    backoff.spin();
                    state = self.state.load(Relaxed);
                }
            }
        }
    }

    /// Spins until the [`Once`] contains a value.
    ///
    /// # Panics
    ///
    /// This function will panic if the [`Once`] previously panicked while attempting
    /// to initialize. This is similar to the poisoning behaviour of `std::sync`'s
    /// primitives.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Once.html#method.wait
    #[inline]
    pub fn wait(&self) {
        if !self.is_completed() {
            self.inner_wait(false);
        }
    }

    /// Blocks the current thread until initialization has completed, ignoring
    /// poisoning.
    ///
    /// If this [`Once`] has been poisoned, this function blocks until it
    /// becomes completed, unlike [`Once::wait()`], which panics in this case.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Once.html#method.wait
    #[inline]
    pub fn wait_force(&self) {
        if !self.is_completed() {
            self.inner_wait(true);
        }
    }

    fn inner_wait(&self, ignore_poisoning: bool) {
        let backoff = crate::utils::Backoff::new();
        let mut state = self.state.load(Relaxed);
        loop {
            match state {
                COMPLETE => {
                    // Ensure visibility
                    core::sync::atomic::fence(Acquire);
                    return;
                }
                POISONED if !ignore_poisoning => {
                    // Panic to propagate the poison.
                    panic!("Once instance has previously been poisoned");
                }
                _ => {
                    backoff.spin();
                    state = self.state.load(Relaxed);
                }
            }
        }
    }

    pub(crate) fn exclusive_state(&mut self) -> OnceExclusiveState {
        match *self.state.get_mut() {
            INCOMPLETE => OnceExclusiveState::Incomplete,
            POISONED => OnceExclusiveState::Poisoned,
            COMPLETE => OnceExclusiveState::Complete,
            _ => unreachable!("invalid Once state"),
        }
    }
}

impl fmt::Debug for Once {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Once").finish_non_exhaustive()
    }
}

/// Fallback implementation of `Once` from the standard library.
///
/// A low-level synchronization primitive for one-time global execution.
///
/// Keep the API consistent with the [standard library].
///
/// [standard library]: https://doc.rust-lang.org/std/sync/struct.OnceLock.html
pub struct OnceLock<T> {
    // FIXME(nonpoison_once): switch to nonpoison version once it is available
    once: Once,
    // Whether or not the value is initialized is tracked by `once.is_completed()`.
    value: UnsafeCell<MaybeUninit<T>>,
    /// `PhantomData` to make sure dropck understands we're dropping T in our Drop impl.
    _marker: PhantomData<T>,
}

impl<T> OnceLock<T> {
    /// Creates a new uninitialized cell.
    #[inline]
    #[must_use]
    pub const fn new() -> OnceLock<T> {
        OnceLock {
            once: Once::new(),
            value: UnsafeCell::new(MaybeUninit::uninit()),
            _marker: PhantomData,
        }
    }

    /// Gets the reference to the underlying value.
    ///
    /// Returns `None` if the cell is uninitialized, or being initialized.
    /// This method never blocks.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.OnceLock.html#method.get
    #[inline]
    pub fn get(&self) -> Option<&T> {
        if self.is_initialized() {
            Some(unsafe { self.get_unchecked() })
        } else {
            None
        }
    }

    /// Gets the mutable reference to the underlying value.
    ///
    /// Returns `None` if the cell is uninitialized.
    ///
    /// This method never blocks.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.OnceLock.html#method.get
    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        if self.is_initialized() {
            Some(unsafe { self.get_unchecked_mut() })
        } else {
            None
        }
    }

    /// Blocks the current thread until the cell is initialized.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.OnceLock.html#method.get
    #[inline]
    pub fn wait(&self) -> &T {
        self.once.wait_force();

        unsafe { self.get_unchecked() }
    }

    /// Initializes the contents of the cell to `value`.
    ///
    /// May block if another thread is currently attempting to initialize the cell. The cell is
    /// guaranteed to contain a value when `set` returns, though not necessarily the one provided.
    ///
    /// Returns `Ok(())` if the cell was uninitialized and
    /// `Err(value)` if the cell was already initialized.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.OnceLock.html#method.set
    #[inline]
    pub fn set(&self, value: T) -> Result<(), T> {
        let mut value = Some(value);
        self.get_or_init(|| value.take().unwrap());
        match value {
            None => Ok(()),
            Some(value) => Err(value),
        }
    }

    /// Gets the contents of the cell, initializing it to `f()` if the cell
    /// was uninitialized.
    ///
    /// Many threads may call `get_or_init` concurrently with different
    /// initializing functions, but it is guaranteed that only one function
    /// will be executed if the function doesn't panic.
    ///
    /// # Panics
    ///
    /// If `f()` panics, the panic is propagated to the caller, and the cell
    /// remains uninitialized.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.OnceLock.html#method.get_or_init
    #[inline]
    pub fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        // Fast path check
        if let Some(value) = self.get() {
            return value;
        }
        self.initialize(f);

        // SAFETY: The inner value has been initialized
        unsafe { self.get_unchecked() }
    }

    fn initialize(&self, f: impl FnOnce() -> T) {
        let slot = &self.value;

        self.once.call_once_force(|_| {
            let value = f();
            unsafe {
                (&mut *slot.get()).write(value);
            }
        });
    }

    /// Consumes the `OnceLock`, returning the wrapped value. Returns
    /// `None` if the cell was uninitialized.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.OnceLock.html#method.into_inner
    #[inline]
    pub fn into_inner(mut self) -> Option<T> {
        self.take()
    }

    /// Takes the value out of this `OnceLock`, moving it back to an uninitialized state.
    ///
    /// Has no effect and returns `None` if the `OnceLock` was uninitialized.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.OnceLock.html#method.take
    #[inline]
    pub fn take(&mut self) -> Option<T> {
        if self.is_initialized() {
            self.once = Once::new();
            unsafe { Some((&*self.value.get()).assume_init_read()) }
        } else {
            None
        }
    }

    #[inline(always)]
    fn is_initialized(&self) -> bool {
        self.once.is_completed()
    }

    #[inline]
    unsafe fn get_unchecked(&self) -> &T {
        debug_assert!(self.is_initialized());
        unsafe { (&*self.value.get()).assume_init_ref() }
    }

    #[inline]
    unsafe fn get_unchecked_mut(&mut self) -> &mut T {
        debug_assert!(self.is_initialized());
        unsafe { (&mut *self.value.get()).assume_init_mut() }
    }
}

unsafe impl<T: Sync + Send> Sync for OnceLock<T> {}
unsafe impl<T: Send> Send for OnceLock<T> {}
impl<T: RefUnwindSafe + UnwindSafe> RefUnwindSafe for OnceLock<T> {}
impl<T: UnwindSafe> UnwindSafe for OnceLock<T> {}

impl<T> Default for OnceLock<T> {
    #[inline]
    fn default() -> OnceLock<T> {
        OnceLock::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for OnceLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_tuple("OnceLock");
        match self.get() {
            Some(v) => d.field(v),
            None => d.field(&format_args!("<uninit>")),
        };
        d.finish()
    }
}

impl<T: Clone> Clone for OnceLock<T> {
    #[inline]
    fn clone(&self) -> OnceLock<T> {
        let cell = Self::new();
        if let Some(value) = self.get() {
            match cell.set(value.clone()) {
                Ok(()) => (),
                Err(_) => unreachable!(),
            }
        }
        cell
    }
}

impl<T> From<T> for OnceLock<T> {
    /// Creates a new cell with its contents set to `value`.
    ///
    /// # Example
    ///
    /// ```
    /// use voker_os::sync::OnceLock;
    ///
    /// # fn main() -> Result<(), i32> {
    /// let a = OnceLock::from(3);
    /// let b = OnceLock::new();
    /// b.set(3)?;
    /// assert_eq!(a, b);
    /// Ok(())
    /// # }
    /// ```
    #[inline]
    fn from(value: T) -> Self {
        let cell = Self::new();
        match cell.set(value) {
            Ok(()) => cell,
            Err(_) => unreachable!(),
        }
    }
}

impl<T: PartialEq> PartialEq for OnceLock<T> {
    /// Equality for two `OnceLock`s.
    ///
    /// Two `OnceLock`s are equal if they either both contain values and their
    /// values are equal, or if neither contains a value.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_os::sync::OnceLock;
    ///
    /// let five = OnceLock::new();
    /// five.set(5).unwrap();
    ///
    /// let also_five = OnceLock::new();
    /// also_five.set(5).unwrap();
    ///
    /// assert!(five == also_five);
    ///
    /// assert!(OnceLock::<u32>::new() == OnceLock::<u32>::new());
    /// ```
    #[inline]
    fn eq(&self, other: &OnceLock<T>) -> bool {
        self.get() == other.get()
    }
}

impl<T: Eq> Eq for OnceLock<T> {}

impl<T> Drop for OnceLock<T> {
    #[inline]
    fn drop(&mut self) {
        if self.is_initialized() {
            // SAFETY: The cell is initialized and being dropped, so it can't
            // be accessed again. We also don't touch the `T` other than
            // dropping it, which validates our usage of #[may_dangle].
            unsafe { (&mut *self.value.get()).assume_init_drop() };
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use alloc::vec::Vec;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;

    use super::{Once, OnceLock};
    use crate::utils::tests::test_unwind_panic;

    // call_once should run exactly once even under contention.
    #[test]
    fn once_runs_only_once() {
        const N: usize = 8;
        let once = Arc::new(Once::new());
        let counter = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..N {
            let o = once.clone();
            let c = counter.clone();
            handles.push(thread::spawn(move || {
                o.call_once(|| {
                    c.fetch_add(1, Ordering::SeqCst);
                });
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    // call_once poison: first panic poisons, later call_once panics again.
    #[test]
    fn once_poison_propagates() {
        let once = Once::new();

        let first = test_unwind_panic(|| {
            once.call_once(|| panic!("boom"));
        });
        assert!(first.is_err());

        let second = test_unwind_panic(|| {
            once.call_once(|| {});
        });
        assert!(second.is_err());
    }

    // call_once_force should recover a poisoned once and mark it complete.
    #[test]
    fn once_call_once_force_recovers() {
        let once = Once::new();
        let state = AtomicUsize::new(0);

        let _ = test_unwind_panic(|| {
            once.call_once(|| panic!("init fail"));
        });

        // Force through poisoning and succeed.
        once.call_once_force(|s| {
            assert!(s.is_poisoned());
            state.store(1, Ordering::SeqCst);
        });

        // Subsequent call_once should be a no-op and not panic.
        once.call_once(|| {
            state.store(2, Ordering::SeqCst);
        });

        assert_eq!(state.load(Ordering::SeqCst), 1);
    }

    // get_or_init should run the initializer exactly once even under contention.
    #[test]
    fn once_lock_init_runs_once() {
        const N: usize = 8;
        let cell = Arc::new(OnceLock::new());
        let init_calls = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..N {
            let c = cell.clone();
            let ic = init_calls.clone();
            handles.push(thread::spawn(move || {
                let v = c.get_or_init(|| {
                    ic.fetch_add(1, Ordering::SeqCst);
                    7u32
                });
                assert_eq!(*v, 7);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(init_calls.load(Ordering::SeqCst), 1);
    }

    // set should fail after initialization and not overwrite the stored value.
    #[test]
    fn once_lock_set_fails_after_init() {
        let cell = OnceLock::new();
        assert!(cell.set(10).is_ok());
        let err = cell.set(20).err().unwrap();
        assert_eq!(err, 20);
        assert_eq!(*cell.get().unwrap(), 10);
    }

    // take should move out the value and reset the cell to an uninitialized state.
    #[test]
    fn once_lock_take_resets() {
        let mut cell = OnceLock::new();
        cell.set("hello").unwrap();
        assert_eq!(cell.take(), Some("hello"));
        assert!(cell.get().is_none());
        // can be initialized again
        cell.set("world").unwrap();
        assert_eq!(cell.get(), Some(&"world"));
    }

}
