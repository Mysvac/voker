#![expect(unsafe_code, reason = "Original implementation")]

use core::cell::UnsafeCell;
use core::fmt;
use core::future::{Future, poll_fn};
use core::marker::PhantomData;
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::task::{Poll, Waker};

use async_task::{Runnable, Task};
use futures_lite::FutureExt;

use voker_utils::extra::BlockList;

// -----------------------------------------------------------------------------
// LocalExecutor

/// A single-threaded executor designed for `!Send` futures.
///
/// Tasks can only be spawned from the thread that created the executor,
/// and task handles must also be used within the same thread.
///
/// ## Single-Threaded Mode
///
/// On no_std and WASM platforms, this executor can handle all tasks directly.
///
/// We provide a global single-threaded executor that:
/// - In no_std environments: blocks and executes tasks directly
/// - In WASM environments: integrates with the JavaScript event loop
///   (also behaving like blocking execution)
///
/// ## Multi-Threaded Mode
///
/// In std environments with multi-threading enabled, we use `thread_local!` to
/// create a `LocalExecutor` for each thread.
///
/// It operates differently in main vs worker threads:
///
/// - **Worker threads**: Created by task pools, they enter an asynchronous loop
///   where the `LocalExecutor` continuously executes tasks until they yield or
///   receive a shutdown signal.
///
/// - **Main thread**: The `LocalExecutor` does not auto-poll and requires explicit ticking.
///   We provide a `tick_local_executor_on_main_thread` function for this purpose.
///
/// Users cannot create `LocalExecutor` instances or obtain references to them directly.
/// Tasks can only be spawned via the task pool's `spawn_local` function, with awareness
/// of the main/worker thread differences mentioned above.
///
/// See more information in [`TaskPool`](crate::TaskPool).
pub struct LocalExecutor<'a> {
    /// Waker used to wake the executor when a new task is scheduled.
    waker: UnsafeCell<Option<Waker>>,
    /// Queue of runnable tasks.
    queue: UnsafeCell<BlockList<Runnable>>,
    /// Marker to tie the executor's lifetime to a single-threaded context.
    _marker: PhantomData<UnsafeCell<&'a ()>>,
}

impl UnwindSafe for LocalExecutor<'_> {}
impl RefUnwindSafe for LocalExecutor<'_> {}

impl fmt::Debug for LocalExecutor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: single thread access
        let len = unsafe { (&*self.queue.get()).len() };
        f.debug_struct("LocalExecutor")
            .field("tasks", &len)
            .finish()
    }
}

impl<'a> LocalExecutor<'a> {
    /// Creates a new, empty `LocalExecutor`.
    pub(crate) const fn new() -> Self {
        Self {
            waker: UnsafeCell::new(None),
            queue: UnsafeCell::new(BlockList::new()),
            _marker: PhantomData,
        }
    }

    /// Spawns a single task onto the executor.
    ///
    /// The returned `Task<T>` can be awaited to retrieve the future's result.
    ///
    /// # Safety
    ///
    /// - The `Runnable` and its future are **not `Send`** and must remain on the executor's thread.
    /// - All `Waker`s created for this task are only valid within the executor thread.
    pub fn spawn<T: 'a>(&self, future: impl Future<Output = T> + 'a) -> Task<T> {
        let queue = &self.queue;
        let waker = &self.waker;

        let schedule = move |runnable: Runnable| {
            // SAFETY: single thread
            unsafe {
                (&mut *queue.get()).push_back(runnable);

                if let Some(a) = (&mut *waker.get()).take() {
                    Waker::wake(a);
                };
            }
        };

        // SAFETY:
        // - If `future` is not [`Send`], [`Runnable`] must be used and dropped on the original
        //   thread.
        // - If `future` is not `'static`, borrowed variables must outlive its [`Runnable`].
        // -`schedule` is not [`Send`] and [`Sync`], all instances of the [`Runnable`]'s [`Waker`]
        //   must be used and dropped on the original thread.
        // - `schedule` is not `'static`, borrowed variables must outlive all instances of the
        //   [`Runnable`]'s [`Waker`].
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

    /// Attempts to run a single task immediately if one is scheduled.
    ///
    /// Returns `true` if a task was executed, `false` otherwise.
    pub fn try_tick(&self) -> bool {
        // SAFETY: single-threaded access
        if let Some(runnable) = unsafe { (&mut *self.queue.get()).pop_front() } {
            runnable.run();
            true
        } else {
            false
        }
    }

    /// Asynchronously runs a single task.
    /// If no tasks are available, the executor will wait until a task is scheduled.
    ///
    /// We separate this from `run` to reduce compilation overhead.
    async fn tick(&self) {
        poll_fn(|ctx| {
            unsafe {
                match &mut *self.waker.get() {
                    Some(waker) => {
                        waker.clone_from(ctx.waker());
                    }
                    None => {
                        *self.waker.get() = Some(ctx.waker().clone());
                    }
                }
            }
            // SAFETY: single thread
            if let Some(runnable) = unsafe { (&mut *self.queue.get()).pop_front() } {
                Poll::Ready(runnable)
            } else {
                Poll::Pending
            }
        })
        .await
        .run();
    }

    /// Continuously runs the executor until the provided future completes.
    ///
    /// The executor polls its own tasks with priority.
    #[inline(always)]
    pub async fn run<T>(&self, stop_signal: impl Future<Output = T>) -> T {
        async {
            loop {
                self.tick().await;
            }
        }
        .or(stop_signal)
        .await
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use alloc::rc::Rc;
    use core::cell::Cell;

    use futures_lite::future::block_on;

    use super::LocalExecutor;

    #[test]
    fn is_unwind_safe() {
        use core::panic::{RefUnwindSafe, UnwindSafe};

        fn assert_unwind_safe<T: UnwindSafe + RefUnwindSafe>() {}
        assert_unwind_safe::<LocalExecutor>();
    }

    #[test]
    fn run_single_task() {
        let ex = LocalExecutor::new();
        let task = ex.spawn(async { 42 });

        let result = block_on(ex.run(async { task.await }));
        assert_eq!(result, 42);
    }

    #[test]
    fn run_double_tasks() {
        let ex = LocalExecutor::new();

        let task1 = ex.spawn(async { 1 });
        let task2 = ex.spawn(async { 2 });

        let results = block_on(ex.run(async { futures_lite::future::zip(task1, task2).await }));

        assert_eq!(results, (1, 2));
    }

    #[test]
    fn try_tick() {
        let ex = LocalExecutor::new();
        assert!(!ex.try_tick());

        let flag = Rc::new(Cell::new(false));
        let flag_clone = flag.clone();

        ex.spawn(async move { flag_clone.set(true) }).detach();

        let executed = ex.try_tick();
        assert!(executed);
        assert!(flag.get());

        // No tasks remaining
        let executed_again = ex.try_tick();
        assert!(!executed_again);
    }

    #[test]
    fn nested_spawning() {
        let ex = LocalExecutor::new();

        let outer_task = ex.spawn(async {
            let inner_result = ex.spawn(async { 100 }).await;
            inner_result * 2
        });

        let result = block_on(ex.run(async { outer_task.await }));
        assert_eq!(result, 200);
    }
}
