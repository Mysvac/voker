#![expect(unsafe_code, reason = "Low level operation")]

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::future::Future;
use core::marker::PhantomData;
use core::mem;

use async_task::Task;
use alloc::sync::Arc;
use voker_os::sync::LazyLock;

use super::{ThreadExecutor, ThreadExecutorTicker, block_on};

// -----------------------------------------------------------------------------
// TaskPoolBuilder

/// Builder for creating a [`TaskPool`].
///
/// No op on the single threaded task pool
#[derive(Default)]
#[must_use]
pub struct TaskPoolBuilder {}

impl TaskPoolBuilder {
    /// Creates a new `TaskPoolBuilder` instance
    #[inline]
    pub const fn new() -> Self {
        Self {}
    }

    /// No op on the single threaded task pool
    #[inline]
    pub fn thread_num(self, _thread_num: usize) -> Self {
        self
    }

    /// No op on the single threaded task pool
    #[inline]
    pub fn stack_size(self, _stack_size: usize) -> Self {
        self
    }

    /// No op on the single threaded task pool
    #[inline]
    pub fn thread_name(self, _thread_name: impl Into<Cow<'static, str>>) -> Self {
        self
    }

    /// No op on the single threaded task pool
    #[inline]
    pub fn on_thread_spawn(self, _f: impl Fn() + Send + Sync + 'static) -> Self {
        self
    }

    /// No op on the single threaded task pool
    #[inline]
    pub fn on_thread_destroy(self, _f: impl Fn() + Send + Sync + 'static) -> Self {
        self
    }

    /// Creates a new [`TaskPool`]
    #[inline]
    #[must_use]
    pub fn build(self) -> TaskPool {
        TaskPool {}
    }
}

// -----------------------------------------------------------------------------
// Static Executor

static LOCAL_EXECUTOR: LazyLock<Arc<ThreadExecutor<'static>>> =
    LazyLock::new(|| Arc::new(ThreadExecutor::new()));

// -----------------------------------------------------------------------------
// TaskPool

/// A single-thread fallback task pool.
///
/// This implementation runs on the current thread only
/// and does not provide background worker threads.
#[derive(Debug, Default)]
pub struct TaskPool {}

impl TaskPool {
    /// Create a `TaskPool` with the default configuration.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        TaskPool {}
    }

    /// Return the number of threads owned by the task pool
    ///
    /// Always return `1` in no_std env.
    #[inline]
    pub fn thread_num(&self) -> usize {
        1
    }

    /// Returns the thread executor for the current thread.
    #[inline]
    pub fn local_executor() -> Arc<ThreadExecutor<'static>> {
        Arc::clone(&LOCAL_EXECUTOR)
    }

    /// Obtains a `Ticker` that can drive the [`ThreadExecutor`]
    /// on the current thread.
    ///
    /// This is typically used on the main thread to explicitly
    /// poll tasks submitted via [`TaskPool::spawn_local`].
    #[inline]
    pub fn local_ticker() -> ThreadExecutorTicker<'static, 'static> {
        LOCAL_EXECUTOR.ticker().unwrap()
    }

    /// Spawns a static future on local thread task queue.
    ///
    /// This is functionally identical to [`TaskPool::spawn`].
    ///
    /// In a `no_std` environment lacking a thread‑local executor,
    /// this function schedules the task on the current thread local executor.
    ///
    /// The caller **must** ensure execution occurs **on the main thread**.
    #[inline]
    pub fn spawn_local<T: 'static>(&self, future: impl Future<Output = T> + 'static) -> Task<T> {
        let task = unsafe { LOCAL_EXECUTOR.spawn_unchecked(future) };
        let ticker = TaskPool::local_ticker();
        // Loop until all tasks are done
        while ticker.try_tick() {}

        task
    }

    /// Spawns a static future onto the thread pool.
    ///
    /// In fallback mode, this method drives the local executor in a loop and
    /// does not return until all currently runnable local tasks are drained.
    ///
    /// This is intentionally synchronous behavior for no-std single-thread use.
    #[inline]
    pub fn spawn<T: Send + 'static>(
        &self,
        future: impl Future<Output = T> + Send + 'static,
    ) -> Task<T> {
        let task = LOCAL_EXECUTOR.spawn(future);
        let ticker = TaskPool::local_ticker();
        // Loop until all tasks are done
        while ticker.try_tick() {}

        task
    }

    /// Allows spawning non-`'static` futures on the thread pool.
    ///
    /// The function takes a callback, passing a scope object into it.
    /// The scope object provided to the callback can be used to spawn
    /// tasks. This function will await the completion of all tasks before
    /// returning.
    ///
    /// This is similar to `rayon::scope` and `crossbeam::scope`
    #[inline]
    pub fn scope<'env, F, T>(&self, f: F) -> Vec<T>
    where
        F: for<'scope> FnOnce(&'scope Scope<'scope, 'env, T>),
        T: Send + 'static,
    {
        self.scope_with(false, None, f)
    }

    /// Allows spawning non-`'static` futures on the thread pool.
    ///
    /// The function takes a callback, passing a scope object into it.
    /// The scope object provided to the callback can be used to spawn
    /// tasks. This function will await the completion of all tasks before
    /// returning.
    ///
    /// This is similar to `rayon::scope` and `crossbeam::scope`
    #[inline]
    pub fn scope_with<'env, F, T>(
        &self,
        _tick_global: bool,
        _remote_executor: Option<&ThreadExecutor>,
        f: F,
    ) -> Vec<T>
    where
        F: for<'scope> FnOnce(&'scope Scope<'scope, 'env, T>),
        T: Send + 'static,
    {
        // SAFETY: This safety comment applies to all references transmuted to 'env.
        //
        // Any futures spawned with these references need to return before this function
        // completes. This is guaranteed because we drive all the futures spawned onto
        // the Scope to completion in this function.
        //
        // However, rust has no way of knowing this so we transmute the lifetimes to 'env
        // here to appease the compiler as it is unable to validate safety.
        //
        // Any usages of the references passed into `Scope` must be accessed through
        // the transmuted reference for the rest of this function.

        // SAFETY: As above, all futures must complete in this function so we can change the lifetime
        let results: RefCell<Vec<Option<T>>> = RefCell::new(Vec::new());
        let results_ref: &'env RefCell<Vec<Option<T>>> = unsafe {
            mem::transmute::<&RefCell<Vec<Option<T>>>, &RefCell<Vec<Option<T>>>>(&results)
        };

        // SAFETY: As above, all futures must complete in this function so we can change the lifetime
        let pending: Cell<usize> = Cell::new(0);
        let pending_tasks: &'env Cell<usize> =
            unsafe { mem::transmute::<&Cell<usize>, &Cell<usize>>(&pending) };

        let mut scope = Scope {
            pending_tasks,
            results_ref,
            scope: PhantomData,
            env: PhantomData,
        };

        // SAFETY: As above, all futures must complete in this function so we can change the lifetime
        let scope_ref: &'env mut Scope<'_, 'env, T> =
            unsafe { mem::transmute::<&mut Scope<T>, &mut Scope<T>>(&mut scope) };

        f(scope_ref);

        // Wait until the scope is complete
        let ticker = LOCAL_EXECUTOR.ticker().unwrap();
        block_on(ticker.run(async {
            while pending_tasks.get() != 0 {
                futures_lite::future::yield_now().await;
            }
        }));

        results.take().into_iter().map(|result| result.unwrap()).collect()
    }
}

// -----------------------------------------------------------------------------
// Scope

/// A `TaskPool` scope for running one or more non-`'static` futures.
///
/// For more information, see [`TaskPool::scope`].
#[derive(Debug)]
pub struct Scope<'scope, 'env: 'scope, T> {
    // The number of pending tasks spawned on the scope
    pending_tasks: &'scope Cell<usize>,
    // Vector to gather results of all futures spawned during scope run
    results_ref: &'env RefCell<Vec<Option<T>>>,
    // make `Scope` invariant over 'scope and 'env
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
}

unsafe impl<T: Send> Send for Scope<'_, '_, T> {}
unsafe impl<T: Send> Sync for Scope<'_, '_, T> {}

impl<'scope, 'env, T: Send + 'env> Scope<'scope, 'env, T> {
    /// Spawns a scoped future onto the executor.
    ///
    /// The scope *must* outlive the provided future. The results of the future
    /// will be returned as a part of [`TaskPool::scope`]'s return value.
    ///
    /// On the single threaded task pool, it just calls [`Scope::spawn_scope`].
    ///
    /// For more information, see [`TaskPool::scope`].
    pub fn spawn<Fut: Future<Output = T> + 'scope + Send>(&self, f: Fut) {
        // increment the number of pending tasks
        let pending_tasks = self.pending_tasks;
        pending_tasks.update(|i| i + 1);

        // add a spot to keep the result, and record the index
        let results_ref = self.results_ref;
        let mut results = results_ref.borrow_mut();
        let task_number = results.len();
        results.push(None);
        drop(results);

        // create the job closure
        let f = async move {
            let result = f.await;

            // store the result in the allocated slot
            let mut results = results_ref.borrow_mut();
            results[task_number] = Some(result);
            drop(results);

            // decrement the pending tasks count
            pending_tasks.update(|i| i - 1);
        };

        unsafe {
            LOCAL_EXECUTOR.spawn_unchecked(f).detach();
        }
    }

    /// Spawns a scoped future onto the executor.
    ///
    /// The scope *must* outlive the provided future. The results of the future
    /// will be returned as a part of [`TaskPool::scope`]'s return value.
    ///
    /// For more information, see [`TaskPool::scope`].
    pub fn spawn_scope<Fut: Future<Output = T> + 'scope + Send>(&self, f: Fut) {
        self.spawn(f);
    }

    /// Spawns a scoped future onto the executor.
    ///
    /// The scope *must* outlive the provided future. The results of the future
    /// will be returned as a part of [`TaskPool::scope`]'s return value.
    ///
    /// On the single threaded task pool, it just calls [`Scope::spawn_scope`].
    ///
    /// For more information, see [`TaskPool::scope`].
    pub fn spawn_remote<Fut: Future<Output = T> + 'scope + Send>(&self, f: Fut) {
        self.spawn(f);
    }
}
