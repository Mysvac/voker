use core::{fmt, time::Duration};

use super::{LockResult, MutexGuard, mutex};
use crate::sync::atomic::{AtomicU32, Ordering::Relaxed};
use crate::time::Instant;
use crate::utils::Futex;

/// Fallback implementation of `WaitTimeoutResult` from the standard library.
///
/// Keep the API consistent with the [standard library].
///
/// [standard library]: https://doc.rust-lang.org/std/sync/struct.WaitTimeoutResult.html
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct WaitTimeoutResult(bool);

impl WaitTimeoutResult {
    /// Returns `true` if the wait was known to have timed out.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.WaitTimeoutResult.html#method.timed_out
    #[must_use]
    pub fn timed_out(&self) -> bool {
        self.0
    }
}

/// Fallback implementation of `Condvar` from the standard library.
///
/// **Important**: [`Condvar::wait_timeout`] and [`Condvar::wait_timeout_while`] depend on [`time::Instant`],
/// please refer to [`crate::time`] to ensure that it can be used in the `no_std` environment.
/// (If you don't call the time function, can ignore it.)
///
/// Condition variables represent the ability to block a thread such that it
/// consumes no CPU time while waiting for an event to occur.
///
/// But due to spinlock implementation, it inevitably make the current thread busy waiting.
///
/// Keep the API consistent with the [standard library].
///
/// [`time::Instant`]: crate::time::Instant
/// [standard library]: https://doc.rust-lang.org/std/sync/struct.Condvar.html
pub struct Condvar {
    state: AtomicU32,
}

impl Condvar {
    /// Creates a new condition variable which is ready to be waited on
    /// and notified.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_os::sync::Condvar;
    ///
    /// let condvar = Condvar::new();
    /// ```
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(0),
        }
    }

    /// Wakes up one blocked thread on this condvar.
    ///
    /// Due to spin-impl, this is equal to [`notify_one`](Self::notify_all).
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Condvar.html#method.notify_one
    #[inline]
    pub fn notify_one(&self) {
        self.state.fetch_add(1, Relaxed);
    }

    /// Wakes up all blocked threads on this condvar.
    ///
    /// Due to spin-impl, this is equal to [`notify_one`](Self::notify_one).
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Condvar.html#method.notify_all
    #[inline]
    pub fn notify_all(&self) {
        self.state.fetch_add(1, Relaxed);
    }

    /// Blocks the current thread until this condition variable receives a
    /// notification.
    ///
    /// This function will atomically unlock the mutex specified (represented by
    /// `guard`) and block the current thread. This means that any calls
    /// to [`notify_one`] or [`notify_all`] which happen logically after the
    /// mutex is unlocked are candidates to wake this thread up. When this
    /// function call returns, the lock specified will have been re-acquired.
    ///
    /// Due to spin-lock implementation, this function always return `Ok`.
    ///
    /// See the [standard library] for further details.
    ///
    /// [`notify_one`]: Self::notify_one
    /// [`notify_all`]: Self::notify_all
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Condvar.html#method.wait
    #[inline]
    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> LockResult<MutexGuard<'a, T>> {
        let lock = mutex::guard_lock(&guard);
        self.wait_optional_timeout(lock, None);
        Ok(guard)
    }

    /// Blocks the current thread until the provided condition becomes false.
    ///
    /// `condition` is checked immediately; if not met (returns `true`), this
    /// will [`wait`] for the next notification then check again. This repeats
    /// until `condition` returns `false`, in which case this function returns.
    ///
    /// This function will atomically unlock the mutex specified (represented by
    /// `guard`) and block the current thread. This means that any calls
    /// to [`notify_one`] or [`notify_all`] which happen logically after the
    /// mutex is unlocked are candidates to wake this thread up. When this
    /// function call returns, the lock specified will have been re-acquired.
    ///
    /// Due to spin-lock implementation, this function will not return `Err`.
    ///
    /// See the [standard library] for further details.
    ///
    /// [`wait`]: Self::wait
    /// [`notify_one`]: Self::notify_one
    /// [`notify_all`]: Self::notify_all
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Condvar.html#method.wait_while
    pub fn wait_while<'a, T, F>(
        &self,
        mut guard: MutexGuard<'a, T>,
        mut condition: F,
    ) -> LockResult<MutexGuard<'a, T>>
    where
        F: FnMut(&mut T) -> bool,
    {
        while condition(&mut *guard) {
            guard = self.wait(guard)?;
        }
        Ok(guard)
    }

    /// Waits on this condition variable for a notification, timing out after a
    /// specified duration.
    ///
    /// The semantics of this function are equivalent to [`wait`] except that
    /// the thread will be blocked for roughly no longer than `dur`. This
    /// method should not be used for precise timing due to anomalies such as
    /// preemption or platform differences that might not cause the maximum
    /// amount of time waited to be precisely `dur`.
    ///
    /// Due to spin-lock implementation, this function will not return `Err`.
    ///
    /// See the [standard library] for further details.
    ///
    /// [`wait`]: Self::wait
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Condvar.html#method.wait_timeout
    #[inline]
    pub fn wait_timeout<'a, T>(
        &self,
        guard: MutexGuard<'a, T>,
        dur: Duration,
    ) -> LockResult<(MutexGuard<'a, T>, WaitTimeoutResult)> {
        let lock = mutex::guard_lock(&guard);
        let result = self.wait_optional_timeout(lock, Some(dur));
        Ok((guard, WaitTimeoutResult(result)))
    }

    /// Waits on this condition variable for a notification, timing out after a
    /// specified duration.
    ///
    /// The semantics of this function are equivalent to [`wait_while`] except
    /// that the thread will be blocked for roughly no longer than `dur`. This
    /// method should not be used for precise timing due to anomalies such as
    /// preemption or platform differences that might not cause the maximum
    /// amount of time waited to be precisely `dur`.
    ///
    /// Due to spin-lock implementation, this function will not return `Err`.
    ///
    /// See the [standard library] for further details.
    ///
    /// [`wait_while`]: Self::wait_while
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Condvar.html#method.wait_timeout_while
    pub fn wait_timeout_while<'a, T, F>(
        &self,
        mut guard: MutexGuard<'a, T>,
        dur: Duration,
        mut condition: F,
    ) -> LockResult<(MutexGuard<'a, T>, WaitTimeoutResult)>
    where
        F: FnMut(&mut T) -> bool,
    {
        let start = Instant::now();
        loop {
            if !condition(&mut *guard) {
                return Ok((guard, WaitTimeoutResult(false)));
            }
            let timeout = match dur.checked_sub(start.elapsed()) {
                Some(timeout) => timeout,
                None => return Ok((guard, WaitTimeoutResult(true))),
            };
            guard = self.wait_timeout(guard, timeout)?.0;
        }
    }

    fn wait_optional_timeout(&self, futex: &Futex, timeout: Option<Duration>) -> bool {
        // Examine the notification counter _before_ we unlock the futex.
        let backoff = crate::utils::Backoff::new();
        let futex_value = self.state.load(Relaxed);

        let mut ret = true;

        // Unlock the futex before going to sleep.
        futex.unlock();

        if let Some(timeout) = timeout {
            let begin = Instant::now();
            'outer: loop {
                // quick path
                if futex_value != self.state.load(Relaxed) {
                    ret = false;
                    break 'outer;
                }
                for _ in 0..10 {
                    backoff.spin();
                    if futex_value != self.state.load(Relaxed) {
                        ret = false;
                        break 'outer;
                    }
                }
                if begin.elapsed() > timeout {
                    break;
                }
            }
        } else {
            while futex_value == self.state.load(Relaxed) {
                backoff.spin();
            }
            ret = false;
        }

        // Lock the futex again.
        futex.lock();

        ret
    }
}

impl fmt::Debug for Condvar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Condvar").finish_non_exhaustive()
    }
}

impl Default for Condvar {
    /// Creates a `Condvar` which is ready to be waited on and notified.
    #[inline]
    fn default() -> Condvar {
        Condvar::new()
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use std::sync::Arc;
    use core::time::Duration;
    use std::sync::mpsc::channel;
    use std::thread;

    use super::Condvar;
    use crate::sync::__fallback::Mutex;

    // notify_one wakes a single waiter
    #[test]
    fn notify_one_wakes_waiter() {
        let cv = Arc::new(Condvar::new());
        let m = Arc::new(Mutex::new(false));
        let (ready_tx, ready_rx) = channel();
        let (done_tx, done_rx) = channel();

        let cv2 = cv.clone();
        let m2 = m.clone();
        thread::spawn(move || {
            let g = m2.lock().unwrap();
            ready_tx.send(()).unwrap(); // about to wait
            let g = cv2.wait(g).unwrap();
            // after wake we expect main to have set it true
            assert!(*g);
            done_tx.send(()).unwrap();
        });

        ready_rx.recv().unwrap();
        {
            let mut g = m.lock().unwrap();
            *g = true;
        }
        cv.notify_one();
        done_rx.recv().unwrap();
    }

    // wait_timeout returns timed out when no notification arrives
    #[test]
    fn wait_timeout_times_out() {
        let cv = Condvar::new();
        let m = Mutex::new(0usize);

        let g = m.lock().unwrap();
        let (_g, res) = cv.wait_timeout(g, Duration::from_millis(20)).unwrap();
        assert!(res.timed_out());
    }

    // wait_while waits until condition becomes false and returns with guard re-acquired
    #[test]
    fn wait_while_obeys_condition_and_wakes() {
        let cv = Arc::new(Condvar::new());
        let m = Arc::new(Mutex::new(false));
        let (tx, rx) = channel();

        // notifier: flip flag and notify after a short sleep
        {
            let cvn = cv.clone();
            let mn = m.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(30));
                let mut g = mn.lock().unwrap();
                *g = true;
                cvn.notify_one();
                tx.send(()).unwrap();
            });
        }

        let g = m.lock().unwrap();
        // wait_while will block until the closure returns false (i.e., *g becomes true -> condition returns false)
        let g = cv.wait_while(g, |v| !*v).unwrap();
        assert!(*g);
        rx.recv().unwrap();
    }

    // notify_all wakes all waiting threads
    #[test]
    fn notify_all_wakes_everyone() {
        const N: usize = 6;
        let cv = Arc::new(Condvar::new());
        let m = Arc::new(Mutex::new(false));
        let (ready_tx, ready_rx) = channel();
        let (done_tx, done_rx) = channel();

        for _ in 0..N {
            let cv2 = cv.clone();
            let m2 = m.clone();
            let rtx = ready_tx.clone();
            let dtx = done_tx.clone();
            thread::spawn(move || {
                let g = m2.lock().unwrap();
                rtx.send(()).unwrap(); // about to wait
                let g = cv2.wait(g).unwrap();
                // ensure guard re-acquired
                drop(g);
                dtx.send(()).unwrap();
            });
        }
        drop(ready_tx);

        // wait for all to be ready
        for _ in 0..N {
            ready_rx.recv().unwrap();
        }

        // flip shared state and notify all
        {
            let mut g = m.lock().unwrap();
            *g = true;
        }
        cv.notify_all();

        // ensure all threads finished
        for _ in 0..N {
            done_rx.recv().unwrap();
        }
    }
}
