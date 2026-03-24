use core::fmt;

use crate::sync::__fallback::{RwLock, RwLockWriteGuard};

// The inner state of a barrier
struct BarrierState {
    count: usize,
    generation_id: usize,
}

/// Fallback implementation of `Barrier` from the standard library.
///
/// A reusable barrier enables multiple threads to synchronize
/// the beginning of some computation.
///
/// Based on spin, which will busy-wait (block) the current thread.
///
/// Keep the API consistent with the [standard library].
///
/// [standard library]: https://doc.rust-lang.org/std/sync/struct.Barrier.html
pub struct Barrier {
    // All threads only need to write when waiting at the beginning.
    // Afterwards, it will become a reader, polling the counter.
    // Using rw-locks can significantly improve performance.
    lock: RwLock<BarrierState>,
    num_threads: usize,
}

/// Fallback implementation of `BarrierWaitResult` from the standard library.
///
/// Keep the API consistent with the [standard library].
///
/// [standard library]: https://doc.rust-lang.org/std/sync/struct.BarrierWaitResult.html
pub struct BarrierWaitResult(bool);

impl fmt::Debug for Barrier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Barrier").finish_non_exhaustive()
    }
}

impl BarrierWaitResult {
    /// Returns `true` if this thread is the "leader thread" for the call to
    /// [`Barrier::wait()`].
    ///
    /// Only one thread will have `true` returned from their result, all other
    /// threads will have `false` returned.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.BarrierWaitResult.html#method.is_leader
    #[must_use]
    #[inline]
    pub fn is_leader(&self) -> bool {
        self.0
    }
}

impl Barrier {
    /// Creates a new barrier that can block a given number of threads.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Barrier.html#method.new
    #[must_use]
    #[inline]
    pub const fn new(n: usize) -> Barrier {
        Barrier {
            lock: RwLock::new(BarrierState {
                count: 0,
                generation_id: 0,
            }),
            num_threads: n,
        }
    }

    /// Blocks the current thread until all threads have rendezvoused here.
    ///
    /// Barriers are re-usable after all threads have rendezvoused once, and can
    /// be used continuously.
    ///
    /// See the [standard library] for further details.
    ///
    /// [standard library]: https://doc.rust-lang.org/std/sync/struct.Barrier.html#method.wait
    pub fn wait(&self) -> BarrierWaitResult {
        // Spin Mutex lock will not fail.
        let mut lock = self.lock.write().unwrap();
        lock.count += 1;

        if lock.count < self.num_threads {
            let backoff = crate::utils::Backoff::new();

            // not the leader
            let local_gen = lock.generation_id;

            let mut lock = RwLockWriteGuard::downgrade(lock);

            while local_gen == lock.generation_id && lock.count < self.num_threads {
                drop(lock);
                backoff.spin();
                lock = self.lock.read().unwrap();
            }
            BarrierWaitResult(false)
        } else {
            // this thread is the leader,
            // and is responsible for incrementing the generation
            lock.count = 0;
            lock.generation_id = lock.generation_id.wrapping_add(1);
            BarrierWaitResult(true)
        }
    }
}

impl fmt::Debug for BarrierWaitResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BarrierWaitResult")
            .field("is_leader", &self.is_leader())
            .finish()
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use alloc::vec::Vec;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::sync::mpsc::{channel, TryRecvError};

    use super::Barrier;

    #[test]
    fn smoke_single_thread_is_leader() {
        let b = Barrier::new(1);
        let r = b.wait();
        assert!(r.is_leader());
    }

    #[test]
    fn test_barrier() {
        const N: usize = 10;

        let barrier = Arc::new(Barrier::new(N));
        let (tx, rx) = channel();

        for _ in 0..N - 1 {
            let c = barrier.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                tx.send(c.wait().is_leader()).unwrap();
            });
        }

        // At this point, all spawned threads should be blocked,
        // so we shouldn't get anything from the port
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));

        let mut leader_found = barrier.wait().is_leader();

        // Now, the barrier is cleared and we should get data.
        for _ in 0..N - 1 {
            if rx.recv().unwrap() {
                assert!(!leader_found);
                leader_found = true;
            }
        }
        assert!(leader_found);
    }

    #[test]
    fn exactly_one_leader_among_threads() {
        const N: usize = 8;
        let b = Arc::new(Barrier::new(N));
        let (tx, rx) = channel();
        for _ in 0..N {
            let b2 = b.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let res = b2.wait();
                tx.send(res.is_leader()).unwrap();
            });
        }
        drop(tx);
        let results: Vec<bool> = rx.iter().collect();
        assert_eq!(results.len(), N);
        assert_eq!(results.iter().filter(|&&x| x).count(), 1);
    }

    #[test]
    fn leader_resets_and_barrier_reusable() {
        const N: usize = 6;
        const ROUNDS: usize = 4;
        let b = Arc::new(Barrier::new(N));
        let leader_count = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..N {
            let b2 = b.clone();
            let lc = leader_count.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..ROUNDS {
                    let r = b2.wait();
                    if r.is_leader() {
                        lc.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(leader_count.load(Ordering::SeqCst), ROUNDS);
    }
}
