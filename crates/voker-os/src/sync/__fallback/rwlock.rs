#![expect(unsafe_code, reason = "RwLock requires unsafe code.")]

use core::fmt;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::sync::atomic::Ordering::{Acquire, Relaxed, Release};

use super::{LockResult, TryLockError, TryLockResult};
use crate::atomic::AtomicU32;

/// Fallback implementation of `RwLock` from the standard library.
///
/// Implementation based on spin-locking, which will busy-wait (block)
/// the current thread until the lock is acquired.
///
/// Keep the API consistent with the [standard library], including
/// [`RwLockWriteGuard::downgrade`] (rust-version >= 1.92).
///
/// # Poisoning
///
/// Although we provide interfaces consistent with the standard library,
/// this type's [`write`] and [`read`] **will never fail** due to spin implementation.
///
/// Even if the thread panic, [`RwLockReadGuard`] and [`RwLockWriteGuard`]
/// can release the lock during [`Drop`] normally.
///
/// See the [standard library] for further details.
///
/// # Note
///
/// This is not suitable for scenarios with excessive concurrent writes,
/// because writes have higher priority than reads, which may cause readers
/// to be continuously cut in line in the queue.
///
/// [`write`]: RwLock::write
/// [`read`]: RwLock::read
/// [standard library]: https://doc.rust-lang.org/std/sync/struct.RwLock.html
pub struct RwLock<T: ?Sized> {
    state: AtomicU32,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}
impl<T: ?Sized> UnwindSafe for RwLock<T> {}
impl<T: ?Sized> RefUnwindSafe for RwLock<T> {}

const READ_LOCKED: u32 = 1;
const MASK: u32 = (1 << 30) - 1;
const WRITE_LOCKED: u32 = MASK;
const DOWNGRADE: u32 = READ_LOCKED.wrapping_sub(WRITE_LOCKED); // READ_LOCKED - WRITE_LOCKED
const MAX_READERS: u32 = MASK - 1;
// When the readers waiting flag is present, it must be actively used by a writer.
const READERS_WAITING: u32 = 1 << 30;
// Unlike the standard library implementation, in this spinlock-based version,
// the presence of the waiting flag guarantees that at least one writer is
// actively spinning and waiting for the lock.
const WRITERS_WAITING: u32 = 1 << 31;

#[inline(always)]
fn is_unlocked(state: u32) -> bool {
    state & MASK == 0
}

#[inline(always)]
fn is_write_locked(state: u32) -> bool {
    state & MASK == WRITE_LOCKED
}

#[inline(always)]
fn has_readers_waiting(state: u32) -> bool {
    state & READERS_WAITING != 0
}

#[inline(always)]
fn has_writers_waiting(state: u32) -> bool {
    state & WRITERS_WAITING != 0
}

#[inline]
fn is_read_lockable(state: u32) -> bool {
    state & MASK < MAX_READERS && !has_readers_waiting(state) && !has_writers_waiting(state)
}

#[inline]
fn has_reached_max_readers(state: u32) -> bool {
    state & MASK == MAX_READERS
}

/// Fallback implementation of `RwLockReadGuard` from the standard library.
///
/// RAII structure used to release the shared read access of a lock when dropped.
///
/// See the [standard library] for further details.
///
/// [standard library]: https://doc.rust-lang.org/std/sync/struct.RwLockReadGuard.html
pub struct RwLockReadGuard<'a, T: 'a + ?Sized> {
    lock: &'a RwLock<T>,
    data: *const T,
}

/// Fallback implementation of `RwLockWriteGuard` from the standard library.
///
/// RAII structure used to release the exclusive write access of a lock when dropped.
///
/// See the [standard library] for further details.
///
/// [standard library]: https://doc.rust-lang.org/std/sync/struct.RwLockWriteGuard.html
pub struct RwLockWriteGuard<'a, T: 'a + ?Sized> {
    lock: &'a RwLock<T>,
    data: *mut T,
}

unsafe impl<T: ?Sized + Sync> Sync for RwLockReadGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for RwLockWriteGuard<'_, T> {}
// impl<T: ?Sized> core::panic::UnwindSafe for RwLockReadGuard<'_, T> {}  // auto implemented
// impl<T: ?Sized> core::panic::UnwindSafe for RwLockWriteGuard<'_, T> {} // auto implemented
impl<T: RefUnwindSafe + ?Sized> RefUnwindSafe for RwLockReadGuard<'_, T> {}
impl<T: RefUnwindSafe + ?Sized> RefUnwindSafe for RwLockWriteGuard<'_, T> {}

impl<T> RwLock<T> {
    /// Creates a new instance of an `RwLock<T>` which is unlocked.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_os::sync::RwLock;
    ///
    /// let lock = RwLock::new(5);
    /// ```
    #[inline]
    pub const fn new(t: T) -> RwLock<T> {
        RwLock {
            state: AtomicU32::new(0),
            data: UnsafeCell::new(t),
        }
    }
}

impl<T: ?Sized> RwLock<T> {
    /// Attempts to acquire this `RwLock` with shared read access.
    ///
    /// If the access could not be granted at this time, then `Err` is returned.
    /// Otherwise, an RAII guard is returned which will release the shared access when it is dropped.
    ///
    /// This function does not block.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.RwLock.html#method.try_read
    #[inline]
    pub fn try_read(&self) -> TryLockResult<RwLockReadGuard<'_, T>> {
        let res = self
            .state
            .fetch_update(Acquire, Relaxed, |s| {
                is_read_lockable(s).then(|| s + READ_LOCKED)
            })
            .is_ok();

        if res {
            Ok(RwLockReadGuard {
                lock: self,
                data: self.data.get(),
            })
        } else {
            Err(TryLockError::WouldBlock)
        }
    }

    /// Attempts to lock this `RwLock` with exclusive write access.
    ///
    /// If the lock could not be acquired at this time, then `Err` is returned.
    /// Otherwise, an RAII guard is returned which will release the lock when it is dropped.
    ///
    /// This function does not block.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.RwLock.html#method.try_write
    #[inline]
    pub fn try_write(&self) -> TryLockResult<RwLockWriteGuard<'_, T>> {
        let res = self
            .state
            .fetch_update(Acquire, Relaxed, |s| {
                is_unlocked(s).then_some(s | WRITE_LOCKED)
            })
            .is_ok();

        if res {
            Ok(RwLockWriteGuard {
                lock: self,
                data: self.data.get(),
            })
        } else {
            Err(TryLockError::WouldBlock)
        }
    }

    /// Locks this `RwLock` with shared read access, blocking the current thread
    /// until it can be acquired.
    ///
    /// Due to spin-lock implementation, this funtion always return `Ok`.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.RwLock.html#method.read
    #[inline]
    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, T>> {
        let state = self.state.load(Relaxed);
        if !is_read_lockable(state)
            || self
                .state
                .compare_exchange_weak(state, state + READ_LOCKED, Acquire, Relaxed)
                .is_err()
        {
            // Lock failed, enter loop.
            self.read_contended();
        }
        Ok(RwLockReadGuard {
            lock: self,
            data: self.data.get(),
        })
    }

    fn read_contended(&self) {
        let backoff = crate::utils::Backoff::new();
        let mut state = self.state.load(Relaxed);

        loop {
            // If we have just been woken up, first check for a `downgrade` call.
            // Otherwise, if we can read-lock it, lock it.
            if is_read_lockable(state) {
                let Err(s) =
                    self.state
                        .compare_exchange_weak(state, state + READ_LOCKED, Acquire, Relaxed)
                else {
                    return; // Locked!
                };
                state = s;
                continue;
            }
            // Check for overflow.
            assert!(
                !has_reached_max_readers(state),
                "too many active read locks on RwLock"
            );

            // Make sure the readers waiting bit is set before we go to sleep.
            if !has_readers_waiting(state)
                && let Err(s) = self.state
                    .compare_exchange(state, state | READERS_WAITING, Relaxed, Relaxed)
            {
                state = s;
                continue;
            }

            backoff.spin();
            // Use Relaxed during loops to reduce bus bandwidth.
            state = self.state.load(Relaxed);
        }
    }

    /// Locks this `RwLock` with exclusive write access, blocking the current
    /// thread until it can be acquired.
    ///
    /// Due to spin-lock implementation, this funtion always return `Ok`.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.RwLock.html#method.write
    #[inline]
    pub fn write(&self) -> LockResult<RwLockWriteGuard<'_, T>> {
        if self
            .state
            .compare_exchange_weak(0, WRITE_LOCKED, Acquire, Relaxed)
            .is_err()
        {
            // Lock failed, enter loop.
            self.write_contended();
        }
        Ok(RwLockWriteGuard {
            lock: self,
            data: self.data.get(),
        })
    }

    fn write_contended(&self) {
        let backoff = crate::utils::Backoff::new();
        let mut state = self.state.load(Relaxed);

        loop {
            // If it's unlocked, we try to lock it.
            if is_unlocked(state) {
                // Spin lock scenario, unable to determine if there is really a write lock waiting.
                // If marked but there are no actual waiter, a deadlock will occur.
                // But it can be unmarked, due to spin, it will still recover normally in the future.
                let target = state & { !WRITERS_WAITING };
                let Err(s) = self.state.compare_exchange_weak(
                    state,
                    target | WRITE_LOCKED,
                    Acquire,
                    Relaxed,
                ) else {
                    return; // Locked!
                };
                state = s;
                continue;
            }

            // Set the waiting bit indicating that we're waiting on it.
            if !has_writers_waiting(state)
                && let Err(s) = self.state
                    .compare_exchange(state, state | WRITERS_WAITING, Relaxed, Relaxed)
            {
                state = s;
                continue;
            }

            backoff.spin();
            // Use Relaxed during loops to reduce bus bandwidth.
            state = self.state.load(Relaxed);
        }
    }

    fn wake_writer_or_readers(&self, mut state: u32) {
        assert!(is_unlocked(state));

        // If only writers are waiting, wake one of them up.
        if state == WRITERS_WAITING {
            let Err(s) = self.state.compare_exchange(state, 0, Relaxed, Relaxed) else {
                return; // Successful, return directly,
            };
            // Maybe some readers are now waiting too. So, continue to the next `if`.
            state = s;
        }

        // If both writers and readers are waiting, leave the readers waiting
        // and only wake up one writer.
        if state == READERS_WAITING + WRITERS_WAITING {
            // For spinlock implementations, resetting the waiting flag is considered
            // equivalent to successfully waking up waiting threads.
            let _ = self
                .state
                .compare_exchange(state, READERS_WAITING, Relaxed, Relaxed);
            // When both readers and writers are waiting, the target state cannot be modified
            // by other threads without violating the lock's invariants.
            return;
        }

        // If readers are waiting, wake them all up.
        if state == READERS_WAITING {
            let _ = self.state.compare_exchange(state, 0, Relaxed, Relaxed);
        }
        // There is no need to deal with the situation where there are only readers at the beginning,
        // but later it becomes a read-write situation. If there are new writers, they will directly occupy the lock.
        // But we still made a judgment to avoid unnecessary atomic operation overhead.
    }

    fn write_unlock(&self) {
        let state = self.state.fetch_sub(WRITE_LOCKED, Release) - WRITE_LOCKED;

        debug_assert!(is_unlocked(state));

        if has_writers_waiting(state) || has_readers_waiting(state) {
            self.wake_writer_or_readers(state);
        }
    }

    fn read_unlock(&self) {
        let state = self.state.fetch_sub(READ_LOCKED, Release) - READ_LOCKED;

        // reader mod, has reader_waiting but without writer_waiting is imposible.
        debug_assert!(!has_readers_waiting(state) || has_writers_waiting(state));

        if is_unlocked(state) && has_writers_waiting(state) {
            self.wake_writer_or_readers(state);
        }
    }

    /// Determines whether the lock is poisoned.
    ///
    /// Due to spin implementation, this function always return false.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.RwLock.html#method.is_poisoned
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
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.RwLock.html#method.clear_poison
    #[inline(always)]
    pub fn clear_poison(&self) {
        // no-op
    }

    /// Consumes this mutex, returning the underlying data.
    ///
    /// Due to spin implementation, this function always return `Ok`.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.RwLock.html#method.into_inner
    #[inline(always)]
    pub fn into_inner(self) -> LockResult<T>
    where
        T: Sized,
    {
        Ok(self.data.into_inner())
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.RwLock.html#method.get_mut
    #[inline(always)]
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        Ok(self.data.get_mut())
    }
}

impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.read_unlock();
    }
}

impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.write_unlock();
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("RwLock");
        match self.try_read() {
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

impl<T: Default> Default for RwLock<T> {
    /// Creates a new `RwLock<T>`, with the `Default` value for T.
    #[inline]
    fn default() -> RwLock<T> {
        RwLock::new(Default::default())
    }
}

impl<T> From<T> for RwLock<T> {
    /// Creates a new instance of an `RwLock<T>` which is unlocked.
    /// This is equivalent to [`RwLock::new`].
    #[inline]
    fn from(t: T) -> Self {
        RwLock::new(t)
    }
}

impl<'rwlock, T: ?Sized> RwLockWriteGuard<'rwlock, T> {
    /// Downgrades a write-locked `RwLockWriteGuard` into a read-locked [`RwLockReadGuard`].
    ///
    /// It must be called through `RwLockWriteGuard::` prefix.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.RwLockWriteGuard.html#method.downgrade
    pub fn downgrade(s: Self) -> RwLockReadGuard<'rwlock, T> {
        let lock = s.lock;

        // We don't want to call the destructor since that calls `write_unlock`.
        core::mem::forget(s);

        let state = lock.state.fetch_add(DOWNGRADE, Release);
        debug_assert!(
            is_write_locked(state),
            "RwLock must be write locked to call `downgrade`"
        );

        if has_readers_waiting(state) {
            // If readers waiting is true, state will not be modified by others.
            lock.state.fetch_sub(READERS_WAITING, Relaxed);
        }

        RwLockReadGuard {
            lock,
            data: lock.data.get(),
        }
    }
}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        // SAFETY: the conditions of `RwLockReadGuard::new` were satisfied when created.
        unsafe { &*self.data }
    }
}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        // SAFETY: the conditions of `RwLockWriteGuard::new` were satisfied when created.
        unsafe { &*self.data }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: the conditions of `RwLockWriteGuard::new` were satisfied when created.
        unsafe { &mut *self.data }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for RwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for RwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

/// copy from standard library
#[cfg(all(test, feature = "std"))]
mod tests {
    use std::fmt::Debug;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::{hint, mem, thread};
    use std::vec::Vec;
    use super::{RwLock, RwLockWriteGuard, RwLockReadGuard, TryLockError};

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
    fn smoke() {
        let l = RwLock::new(());
        drop(l.read().unwrap());
        drop(l.write().unwrap());
        drop((l.read().unwrap(), l.read().unwrap()));
        drop(l.write().unwrap());
    }

    #[test]
    fn frob() {
        const N: u32 = 10;
        const M: usize = if cfg!(miri) { 100 } else { 1000 };

        let r = Arc::new(RwLock::new(()));

        let (tx, rx) = channel::<()>();
        for i in 0..N {
            let tx = tx.clone();
            let r = r.clone();
            thread::spawn(move || {
                // Use system time and thread ID to create pseudo-randomness
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                
                // Mix thread ID and timestamp for better distribution
                let mut seed = ((now as u64).wrapping_add(i as u64 * 2654435761)) as u32;
                
                for _ in 0..M {
                    // Simple LCG pseudo-random generator
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    
                    // Use low bits to decide read/write (approximately 1/N probability for write)
                    if (seed & ((1 << 16) - 1)) < (65536 / N as u32) {
                        drop(r.write().unwrap());
                    } else {
                        drop(r.read().unwrap());
                    }
                }
                drop(tx);
            });
        }
        drop(tx);
        let _ = rx.recv();
    }

    #[test]
    fn test_rw_arc() {
        let arc = Arc::new(RwLock::new(0));
        let arc2 = arc.clone();
        let (tx, rx) = channel();

        thread::spawn(move || {
            let mut lock = arc2.write().unwrap();
            for _ in 0..10 {
                let tmp = *lock;
                *lock = -1;
                thread::yield_now();
                *lock = tmp + 1;
            }
            tx.send(()).unwrap();
        });

        // Readers try to catch the writer in the act
        let mut children = Vec::new();
        for _ in 0..5 {
            let arc3 = arc.clone();
            children.push(thread::spawn(move || {
                let lock = arc3.read().unwrap();
                assert!(*lock >= 0);
            }));
        }

        // Wait for children to pass their asserts
        for r in children {
            assert!(r.join().is_ok());
        }

        // Wait for writer to finish
        rx.recv().unwrap();
        let lock = arc.read().unwrap();
        assert_eq!(*lock, 10);
    }

    #[test]
    fn test_rwlock_unsized() {
        let rw: &RwLock<[i32]> = &RwLock::new([1, 2, 3]);
        {
            let b = &mut *rw.write().unwrap();
            b[0] = 4;
            b[2] = 5;
        }
        let comp: &[i32] = &[4, 2, 5];
        assert_eq!(&*rw.read().unwrap(), comp);
    }

    #[test]
    fn test_into_inner() {
        let m = RwLock::new(NonCopy(10));
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
        let m = RwLock::new(Foo(num_drops.clone()));
        assert_eq!(num_drops.load(Ordering::SeqCst), 0);
        {
            let _inner = m.into_inner().unwrap();
            assert_eq!(num_drops.load(Ordering::SeqCst), 0);
        }
        assert_eq!(num_drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_get_mut() {
        let mut m = RwLock::new(NonCopy(10));
        *m.get_mut().unwrap() = NonCopy(20);
        assert_eq!(m.into_inner().unwrap(), NonCopy(20));
    }

    #[test]
    fn test_rwlock_try_write() {
        let lock = RwLock::new(0isize);
        let read_guard = lock.read().unwrap();

        let write_result = lock.try_write();
        match write_result {
            Err(TryLockError::WouldBlock) => (),
            Ok(_) => panic!("try_write should not succeed while read_guard is in scope"),
            Err(_) => panic!("unexpected error"),
        }

        drop(read_guard);
    }

    #[test]
    fn test_downgrade_basic() {
        let r = RwLock::new(());
        let write_guard = r.write().unwrap();
        let _read_guard = RwLockWriteGuard::downgrade(write_guard);
    }

    #[test]
    fn test_downgrade_observe() {
        // Inspired by the test `test_rwlock_downgrade` from:
        // https://github.com/Amanieu/parking_lot/blob/master/src/rwlock.rs

        const W: usize = 20;
        const N: usize = if cfg!(miri) { 40 } else { 100 };

        // This test spawns `W` writer threads, where each will increment a counter `N` times,
        // ensuring that the value they wrote has not changed after downgrading.

        let rw = Arc::new(RwLock::new(0));

        // Spawn the writers that will do `W * N` operations and checks.
        let handles: Vec<_> = (0..W)
            .map(|_| {
                let rw = rw.clone();
                thread::spawn(move || {
                    for _ in 0..N {
                        // Increment the counter.
                        let mut write_guard = rw.write().unwrap();
                        *write_guard += 1;
                        let cur_val = *write_guard;

                        // Downgrade the lock to read mode, where the value protected cannot be
                        // modified.
                        let read_guard = RwLockWriteGuard::downgrade(write_guard);
                        assert_eq!(cur_val, *read_guard);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(*rw.read().unwrap(), W * N);
    }

    #[test]
    fn test_downgrade_atomic() {
        const NEW_VALUE: i32 = -1;

        // This test checks that `downgrade` is atomic, meaning as soon as a write lock has been
        // downgraded, the lock must be in read mode and no other threads can take the write lock to
        // modify the protected value.

        // `W` is the number of evil writer threads.
        const W: usize = 20;
        let rwlock = Arc::new(RwLock::new(0));

        // Spawns many evil writer threads that will try and write to the locked value before the
        // initial writer (who has the exclusive lock) can read after it downgrades.
        // If the `RwLock` behaves correctly, then the initial writer should read the value it wrote
        // itself as no other thread should be able to mutate the protected value.

        // Put the lock in write mode, causing all future threads trying to access this go to sleep.
        let mut main_write_guard = rwlock.write().unwrap();

        // Spawn all of the evil writer threads. They will each increment the protected value by 1.
        let handles: Vec<_> = (0..W)
            .map(|_| {
                let rwlock = rwlock.clone();
                thread::spawn(move || {
                    // Will go to sleep since the main thread initially has the write lock.
                    let mut evil_guard = rwlock.write().unwrap();
                    *evil_guard += 1;
                })
            })
            .collect();

        // Wait for a good amount of time so that evil threads go to sleep.
        // Note: this is not strictly necessary...
        let eternity = std::time::Duration::from_millis(42);
        thread::sleep(eternity);

        // Once everyone is asleep, set the value to `NEW_VALUE`.
        *main_write_guard = NEW_VALUE;

        // Atomically downgrade the write guard into a read guard.
        let main_read_guard = RwLockWriteGuard::downgrade(main_write_guard);

        // If the above is not atomic, then it would be possible for an evil thread to get in front
        // of this read and change the value to be non-negative.
        assert_eq!(*main_read_guard, NEW_VALUE, "`downgrade` was not atomic");

        // Drop the main read guard and allow the evil writer threads to start incrementing.
        drop(main_read_guard);

        for handle in handles {
            handle.join().unwrap();
        }

        let final_check = rwlock.read().unwrap();
        assert_eq!(*final_check, W as i32 + NEW_VALUE);
    }

    #[test]
    fn test_read_guard_covariance() {
        fn do_stuff<'a, 'b>(_: RwLockReadGuard<'_, &'a i32>, _: &'b i32) {}
        let j: i32 = 5;
        let lock = RwLock::new(&j);
        {
            let i = 6;
            do_stuff(lock.read().unwrap(), &i);
        }
        drop(lock);
    }


    #[cfg(panic = "unwind")]
    #[test]
    fn test_rw_arc_access_in_unwind() {
        use crate::utils::tests::test_thread_panic;

        let arc = Arc::new(RwLock::new(1));
        let arc2 = arc.clone();
        
        // Only suppress panic output inside the spawned thread
        let _ = test_thread_panic(move || {
            struct Unwinder {
                i: Arc<RwLock<isize>>,
            }
            impl Drop for Unwinder {
                fn drop(&mut self) {
                    let mut lock = self.i.write().unwrap();
                    *lock += 1;
                }
            }
            let _u = Unwinder { i: arc2 };
            panic!();
        });
        
        // Our implementation should not be poisoned, lock should be 2
        let lock = arc.read().unwrap();
        assert_eq!(*lock, 2);
    }
}
