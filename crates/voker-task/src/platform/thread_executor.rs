#![expect(unsafe_code, reason = "task spawn_unchecked is unsafe")]

use core::fmt;
use core::future::poll_fn;
use core::marker::PhantomData;
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::task::Poll;

use async_task::{Runnable, Task};
use atomic_waker::AtomicWaker;

use futures_lite::FutureExt;
use voker_os::thread::thread_hash;
use voker_os::utils::SegQueue;

// -----------------------------------------------------------------------------
// Thread Executor

/// A thread local storaged executor,
///
/// Can spawn `Send` tasks from other threads,
/// but can only be ticked on the thread it was created on.
///
/// See more information in [`TaskPool`](crate::TaskPool);
///
/// # Example
///
/// ```
/// # use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
/// use voker_task::{ThreadExecutor, TaskPool};
///
/// let executor: Arc<ThreadExecutor> = TaskPool::local_executor();
/// let count = Arc::new(AtomicU32::new(0));
///
/// // create some owned values that can be moved into another thread
/// let count_clone = count.clone();
///
/// std::thread::scope(|scope| {
///     scope.spawn(|| {
///         // we cannot get the ticker from another thread
///         let not_ticker = executor.ticker();
///         assert!(not_ticker.is_none());
///
///         // but we can spawn tasks from another thread
///         executor.spawn(async move {
///             count_clone.fetch_add(1, Ordering::Relaxed);
///         }).detach();
///     });
/// });
///
/// // the tasks do not make progress unless the executor is manually ticked
/// assert_eq!(count.load(Ordering::Relaxed), 0);
///
/// // tick the ticker until task finishes
/// let thread_ticker = executor.ticker().unwrap();
/// thread_ticker.try_tick();
/// assert_eq!(count.load(Ordering::Relaxed), 1);
/// ```
pub struct ThreadExecutor<'a> {
    // A thread-safe MPSC queue for cross-thread task submission.
    queue: SegQueue<Runnable>,
    // Waker used to wake the ticker when new tasks are scheduled.
    waker: AtomicWaker,
    // The thread on which this executor was created.
    thread: u64,
    // Makes the `'a` lifetime invariant.
    _marker: PhantomData<&'a ()>,
}

unsafe impl Send for ThreadExecutor<'_> {}
unsafe impl Sync for ThreadExecutor<'_> {}
impl UnwindSafe for ThreadExecutor<'_> {}
impl RefUnwindSafe for ThreadExecutor<'_> {}

impl<'task> ThreadExecutor<'task> {
    pub(super) fn new() -> Self {
        Self {
            queue: SegQueue::new(),
            waker: AtomicWaker::new(),
            thread: thread_hash(),
            _marker: PhantomData,
        }
    }

    pub(super) unsafe fn spawn_unchecked<T: 'task>(
        &self,
        future: impl Future<Output = T> + 'task,
    ) -> Task<T> {
        let queue = &self.queue;
        let waker = &self.waker;

        let schedule = move |runnable: Runnable| {
            queue.push(runnable);
            waker.wake();
        };

        let (runnable, task) = unsafe {
            crate::cfg::std! {
                if {
                    async_task::Builder::new()
                        .propagate_panic(true)
                        .spawn_unchecked(|()|future, schedule)
                } else {
                    async_task::spawn_unchecked(future, schedule)
                }
            }
        };

        runnable.schedule();

        task
    }

    pub(super) unsafe fn ticker_unchecked<'ticker>(
        &'ticker self,
    ) -> ThreadExecutorTicker<'task, 'ticker> {
        ThreadExecutorTicker {
            executor: self,
            _marker: PhantomData,
        }
    }

    /// Spawn a task on the thread executor
    #[inline]
    pub fn spawn<T: Send + 'task>(
        &self,
        future: impl Future<Output = T> + Send + 'task,
    ) -> Task<T> {
        unsafe { self.spawn_unchecked(future) }
    }

    /// Gets the [`ThreadExecutorTicker`] for this executor.
    ///
    /// It only returns the ticker if it's on the thread the executor
    /// was created on and returns `None` otherwise.
    #[inline]
    pub fn ticker<'ticker>(&'ticker self) -> Option<ThreadExecutorTicker<'task, 'ticker>> {
        if thread_hash() != self.thread {
            return None;
        }

        Some(unsafe { self.ticker_unchecked() })
    }

    /// Returns true if `self` and `other`'s executor is same
    #[inline(always)]
    pub fn is_same(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

impl<'a> fmt::Debug for ThreadExecutor<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThreadExecutor")
            .field("thread", &self.thread)
            .field("tasks", &self.queue.len())
            .finish()
    }
}

// -----------------------------------------------------------------------------
// Executor Ticker

/// Used to tick the [`ThreadExecutor`].
///
/// Created from:
/// - [`ThreadExecutor::ticker`]
/// - [`TaskPool::local_ticker`]
///
/// [`TaskPool::local_ticker`]: crate::TaskPool::local_ticker
#[derive(Debug)]
pub struct ThreadExecutorTicker<'task, 'ticker> {
    executor: &'ticker ThreadExecutor<'task>,
    // make type !Send and !Sync
    _marker: PhantomData<*const ()>,
}

impl<'task, 'ticker> ThreadExecutorTicker<'task, 'ticker> {
    /// Tick the thread executor.
    pub async fn tick(&self) {
        poll_fn(|ctx| {
            self.executor.waker.register(ctx.waker());

            match self.executor.queue.pop() {
                Some(r) => Poll::Ready(r),
                None => Poll::Pending,
            }
        })
        .await
        .run();
    }

    /// Tick the thread executor until receive stop signal.
    pub async fn run<T>(&self, stop_signal: impl Future<Output = T>) -> T {
        let tick_forever = async {
            loop {
                self.tick().await;
            }
        };

        tick_forever.or(stop_signal).await
    }

    /// Synchronously try to tick a task on the executor.
    /// Returns false if does not find a task to tick.
    #[inline]
    pub fn try_tick(&self) -> bool {
        match self.executor.queue.pop() {
            Some(runnable) => {
                runnable.run();
                true
            }
            None => false,
        }
    }
}
