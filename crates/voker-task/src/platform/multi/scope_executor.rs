//! This module provides the implementation of `ScopeExecutor`,
//! 
//! The executor system employs a three-tier design:
//! 
//! - `GlobalExecutor`: Global, work-stealing executor shared across all threads in a pool
//! - `LocalExecutor`: Per-thread executor for thread-local tasks
//! - `ScopeExecutor`: Per-thread executor for scoped tasks or cross-thread transfers
#![expect(unsafe_code, reason = "original implementation")]

use core::fmt;
use core::marker::PhantomData;
use core::cell::UnsafeCell;
use core::panic::{UnwindSafe, RefUnwindSafe};
use core::future::poll_fn;
use core::task::Poll;

use std::thread::{self, ThreadId};

use async_task::{Task, Runnable};
use atomic_waker::AtomicWaker;

use voker_os::utils::ListQueue;
use voker_os::utils::CachePadded;
use voker_utils::extra::ArrayDeque;

// -----------------------------------------------------------------------------
// Scope Executor

/// A **thread-affine** multi-producer, single-consumer async executor.
///
/// `ScopeExecutor` is designed to be used inside [`TaskPool::scope`] for **scoped
/// parallelism**.
///
/// It allows tasks to be spawned from **any thread**, but enforces that **all task
/// execution happens on the thread where the executor was created**.
///
/// ## Execution Model
///
/// - Tasks may be spawned from multiple threads (MPSC).
/// - Tasks are stored in a thread-safe queue.
/// - Only the owning thread may **tick** the executor and execute tasks.
/// - Execution is explicit: the executor does not run in the background.
///
/// ## Single-Threaded Environments
///
/// In `no_std` or WASM environments, `LocalExecutor` is used instead.
/// `ScopeExecutor` is unavailable and accessing it will panic.
///
/// ## Multi-Threaded Environments
///
/// Each thread owns **one** `ScopeExecutor`, stored in thread-local storage.
///
/// Unlike `LocalExecutor`, a `ScopeExecutor` does **not automatically drive itself**.
/// The owning thread must explicitly call `tick()` via a [`ScopeExecutorTicker`].
///
/// ## Purpose
///
/// `ScopeExecutor` fills the gap between:
///
/// - `LocalExecutor`: thread-local, non-`Send`, cannot receive tasks from other threads.
/// - `GlobalExecutor`: global, load-balanced, but with no guarantee which thread executes a task.
///
/// It enables **thread-directed scheduling**:
///
/// - Tasks can be *sent* to a specific thread.
/// - Execution remains deterministic and thread-affine.
///
/// ## Typical Use Cases
///
/// - Scoped parallelism with deterministic thread assignment.
/// - Running tasks on the main thread from worker threads.
/// - Integrating async tasks into thread-bound event loops.
///
/// [`TaskPool::scope`]: crate::TaskPool::scope
pub struct ScopeExecutor<'a> {
    // A small thread-local cache to reduce contention on the shared queue.
    //
    // We choose cache `14` elements because `Runnable` is 8 bytes.
    // Then the size of ArrayDeque is 16 * 8 = 128 (including `len` and `tail_index`).
    cache: CachePadded<UnsafeCell<ArrayDeque<Runnable, 14>>>,
    // A thread-safe MPSC queue for cross-thread task submission.
    queue: ListQueue<Runnable>,
    // Waker used to wake the ticker when new tasks are scheduled.
    // 
    // Shared cache line with `thread_id` is acceptable because `thread_id`
    // is rarely accessed and does not participate in hot-path updates.
    waker: AtomicWaker,
    // The thread on which this executor was created.
    thread_id: ThreadId,
    _marker: PhantomData<&'a ()>,
}

unsafe impl Send for ScopeExecutor<'_> {}
unsafe impl Sync for ScopeExecutor<'_> {}
impl UnwindSafe for ScopeExecutor<'_> {}
impl RefUnwindSafe for ScopeExecutor<'_> {}

impl<'task> ScopeExecutor<'task> {
    /// create a new [`ScopeExecutor`]
    pub(super) fn new() -> Self {
        Self {
            queue: ListQueue::default(),
            cache: CachePadded::new(UnsafeCell::new(ArrayDeque::new())),
            waker: AtomicWaker::new(),
            thread_id: thread::current().id(),
            _marker: PhantomData,
        }
    }

    /// Fetches the next runnable task.
    ///
    /// This first attempts to pop from the thread-local cache. If the cache
    /// is empty, it refills from the shared queue.
    ///
    /// # Safety
    ///
    /// Must only be called on the thread where this executor was created.
    /// This function relies on thread-affinity to safely access internal
    /// mutable state without synchronization.
    #[inline]
    unsafe fn get_runnable(&self) -> Option<Runnable> {
        if let Some(runnable) = unsafe{ 
            (&mut *self.cache.get()).pop_front()
        } {
            // quick path
            Some(runnable)
        } else {
            // SAFETY: be called on the thread
            // where this executor was created.
            unsafe{ self.get_runnable_from_queue() }
        }
    }

    /// Slow path: refill the local cache from the shared queue.
    ///
    /// # Safety
    ///
    /// Must be called on the owning thread.
    #[cold]
    unsafe fn get_runnable_from_queue(&self) -> Option<Runnable> {
        let cache = unsafe{ &mut *self.cache.get() };

        // pop from queue
        let mut lock = self.queue.lock_pop();
        let runnable = self.queue.pop_with_lock(&mut lock)?;

        // move `Runnable` from queue to local cache
        while !cache.is_full() {
            let Some(item) = self.queue.pop_with_lock(&mut lock) else {
                return Some(runnable);
            };
            // SAFETY: !is_full()
            unsafe{ cache.push_back_unchecked(item); }
        }
        Some(runnable)
    }

    /// Spawns a task onto this executor.
    ///
    /// The task may be spawned from **any thread**, but will always be
    /// executed on the executor's owning thread.
    ///
    /// Task execution only begins once the executor is ticked.
    ///
    /// # Panics
    ///
    /// Panics during task execution are propagated to the returned [`Task`].
    pub fn spawn<T: Send + 'task>(
        &self,
        future: impl Future<Output = T> + Send + 'task,
    ) -> Task<T> {
        let queue = &self.queue;
        let waker = &self.waker;

        let schedule = move |runnable: Runnable| {
            queue.push(runnable);
            waker.wake();
        };

        // SAFETY:
        // - `future` is [`Send`].
        // - Self is `thread_local`, so schedule is to `'static`.
        // - If `future` is not `'static`, borrowed variables must outlive its [`Runnable`].
        let (runnable, task) = unsafe {
            async_task::Builder::new()
                .propagate_panic(true)
                .spawn_unchecked(|()|future, schedule)
        };

        runnable.schedule();

        task
    }

    /// Returns a ticker that can drive this executor.
    ///
    /// The ticker is only available on the thread where the executor was
    /// created. Calling this method from any other thread returns `None`.
    ///
    /// This enforces thread-affinity both at runtime and at the type level:
    /// the returned [`ScopeExecutorTicker`] is neither `Send` nor `Sync`.
    #[inline]
    pub fn ticker<'ticker>(&'ticker self) -> Option<ScopeExecutorTicker<'task, 'ticker>> {
        if thread::current().id() == self.thread_id {
            Some(ScopeExecutorTicker {
                executor: self,
                _marker: PhantomData,
            })
        } else {
            None
        }
    }

    /// Returns true if `self` and `other`'s executor is same
    #[inline(always)]
    pub fn is_same(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

impl<'a> fmt::Debug for ScopeExecutor<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopeExecutor")
            .field("thread_id", &self.thread_id)
            .finish()
    }
}

// -----------------------------------------------------------------------------
// Scope Executor Ticker

/// A handle used to explicitly **drive a `ScopeExecutor`** forward.
///
/// A `ScopeExecutor` does **not** make progress unless it is explicitly ticked.
/// This type enforces thread-affinity at the type level and cannot be sent
/// or shared across threads.
///
/// Typically used inside an async loop or a thread-bound event loop.
#[derive(Debug)]
pub struct ScopeExecutorTicker<'task, 'ticker> {
    executor: &'ticker ScopeExecutor<'task>,
    // make type not send or sync
    _marker: PhantomData<*const ()>,
}

impl<'task, 'ticker> ScopeExecutorTicker<'task, 'ticker> {
    /// Polls and executes a single task asynchronously.
    ///
    /// If no task is available, this method registers the current waker and
    /// returns `Pending`. The waker will be notified when new work arrives.
    #[inline]
    pub async fn tick(&self) {
        poll_fn(|ctx| {
            self.executor.waker.register(ctx.waker());

            // SAFETY: call on the thread that Executor is initialzied on. 
            if let Some(runnable) = unsafe {
                self.executor.get_runnable()
            } {
                Poll::Ready(runnable)
            } else {
                Poll::Pending
            }
        }).await.run();
    }

    /// Attempts to synchronously execute a single task.
    ///
    /// Returns `false` if no task is available.
    ///
    /// This is useful for integration with non-async event loops.
    #[inline]
    pub fn try_tick(&self) -> bool {
        // SAFETY: call on the thread that Executor is initialzied on. 
        if let Some(runnable) = unsafe{ 
            self.executor.get_runnable()
        } {
            runnable.run();
            true
        } else {
            false
        }
    }
}



