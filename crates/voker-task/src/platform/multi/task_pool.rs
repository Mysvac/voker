
use alloc::vec::Vec;
use alloc::boxed::Box;
use alloc::format;
use core::any::Any;
use core::future::Future;
use core::marker::PhantomData;
use core::mem;
use core::panic::AssertUnwindSafe;
use alloc::borrow::Cow;
use std::thread;
use std::thread::JoinHandle;

use voker_os::thread::available_parallelism;
use voker_os::utils::SegQueue;
use voker_os::sync::Arc;
use futures_lite::FutureExt;
use async_task::FallibleTask;

use async_task::Task;
use super::GlobalExecutor;
use super::{ThreadExecutor, ThreadExecutorTicker};
use super::block_on;

// -----------------------------------------------------------------------------
// OnDrop

struct CallOnDrop(Option<Arc<dyn Fn() + Send + Sync + 'static>>);

impl Drop for CallOnDrop {
    fn drop(&mut self) {
        if let Some(call) = self.0.as_ref() {
            call();
        }
    }
}

// -----------------------------------------------------------------------------
// TaskPoolBuilder

/// Builder for creating a [`TaskPool`].
///
/// Currently configurable parameters:
///
/// - [`thread_num`]: Number of additional worker threads to spawn (excluding the current thread).
///   Defaults to the number of logical cores on the system.
///
/// - [`thread_name`]: Thread name prefix. If set, threads are named in the format
///   `{thread_name} {id}`, e.g., `computor 1`. Default: `TaskPool {id}`.
///
/// - [`stack_size`]: Stack size for additional threads. Default is system-dependent.
///
/// - [`on_thread_spawn`]: Callback executed once when each thread spawns.
///
/// - [`on_thread_destroy`]: Callback executed once when each thread is about to terminate.
///
/// # Examples
///
/// ```
/// use voker_task::TaskPoolBuilder;
/// use std::sync::atomic::{AtomicU32, Ordering};
///
/// let task_pool = TaskPoolBuilder::new()
///     .thread_num(2)
///     .thread_name("doc")
///     .build();
///
/// let result = AtomicU32::new(0);
///
/// task_pool.scope(|scope| {
///     for _ in 0..100 {
///         scope.spawn(async {
///             result.fetch_add(1, Ordering::Relaxed);
///         })
///     }
/// });
///
/// let result = result.load(Ordering::Relaxed);
/// assert_eq!(result, 100);
/// ```
///
/// [`thread_num`]: Self::thread_num
/// [`thread_name`]: Self::thread_name
/// [`stack_size`]: Self::stack_size
/// [`on_thread_spawn`]: Self::on_thread_spawn
/// [`on_thread_destroy`]: Self::on_thread_destroy
#[derive(Default)]
#[must_use]
pub struct TaskPoolBuilder {
    /// Number of threads. If `None`, uses logical core count.
    thread_num: Option<usize>,
    /// Custom stack size.
    stack_size: Option<usize>,
    /// Thread name prefix.
    thread_name: Option<Cow<'static, str>>,
    /// Called on thread spawn.
    on_thread_spawn: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
    /// Called on thread termination.
    on_thread_destroy: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
}

impl TaskPoolBuilder {
    /// Creates a new [`TaskPoolBuilder`].
    #[inline]
    pub const fn new() -> Self {
        Self{
            thread_num: None,
            stack_size: None,
            thread_name: None,
            on_thread_spawn: None,
            on_thread_destroy: None,
        }
    }

    /// Sets the number of threads in the pool.
    ///
    /// If unset, defaults to the system's logical core count.
    #[inline]
    pub fn thread_num(mut self, thread_num: usize) -> Self {
        self.thread_num = Some(thread_num);
        self
    }

    /// Override the stack size of the threads created for the pool.
    #[inline]
    pub fn stack_size(mut self, stack_size: usize) -> Self {
        self.stack_size = Some(stack_size);
        self
    }

    /// Sets the thread name prefix.
    ///
    /// Threads will be named `<thread_name> (<thread_index>)`, e.g., `MyThreadPool (2)`.
    #[inline]
    pub fn thread_name(mut self, thread_name: impl Into<Cow<'static, str>>) -> Self {
        self.thread_name = Some(thread_name.into());
        self
    }

    /// Sets a callback invoked once per thread when it starts.
    ///
    /// Executed on the thread itself with access to thread‑local storage.
    /// Blocks async task execution on that thread until the callback completes.
    #[inline]
    pub fn on_thread_spawn(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        let arc = Arc::new(f);

        self.on_thread_spawn = Some(arc);
        self
    }

    /// Sets a callback invoked once per thread when it terminates.
    ///
    /// Executed on the thread itself with access to thread‑local storage.
    /// Blocks thread termination until the callback completes.
    #[inline]
    pub fn on_thread_destroy(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        let arc = Arc::new(f);

        self.on_thread_destroy = Some(arc);
        self
    }

    /// Creates a [`TaskPool`] with the configured options.
    #[inline]
    #[must_use]
    pub fn build(self) -> TaskPool {
        TaskPool::new_internal(self)
    }
}

// -----------------------------------------------------------------------------
// TaskPool

std::thread_local! {
    static THREAD_EXECUTOR: Arc<ThreadExecutor<'static>> = Arc::new(ThreadExecutor::new());
}

/// A thread pool for executing asynchronous tasks.
/// 
/// Manages multi-threaded resources and schedules asynchronous workloads.
/// Note that `0` threads are feasible, then all tasks run on the main thread.
///
/// ---
///
/// # Core APIs
///
/// The pool provides four primary interfaces:
///
/// - [`TaskPool::spawn`]
/// - [`TaskPool::spawn_local`]
/// - [`TaskPool::scope`]
/// - [`TaskPool::scope_with`]
///
/// The `spawn` family requires `'static` tasks, while `scope` supports non‑`'static` tasks.
///
/// Specifically:
/// - `spawn_scope` accepts non‑`Send` tasks.
/// - `scope_with` allows sending tasks to a specific target thread.
///
/// ## `spawn` APIs
///
/// `spawn` is the most commonly used API. Tasks submitted via `spawn` are automatically
/// distributed across available worker threads with work-stealing load balancing.
///
/// `spawn_scope` is designed for thread-local tasks (e.g., ECS plugin initialization).
/// - When called from the main thread, spawned tasks are **not** automatically polled
///   and require explicit driving via [`TaskPool::local_ticker`].
/// - When called from a worker thread, spawned tasks are automatically executed.
///
/// Both `spawn` and `spawn_scope` return [`Task`] handles, which are futures themselves.
/// They can be awaited with [`block_on`] or composed with other async utilities.
///
/// ## `scope` APIs
///
/// [`Scope::spawn`] behaves like [`TaskPool::spawn`]: tasks are submitted to the global
/// executor and automatically distributed across worker threads.
///
/// [`Scope::spawn_scope`] is analogous to [`TaskPool::spawn_local`] which forces the task
/// to stay on the current thread. Unlike the pool version, tasks spawned with
/// `Scope::spawn_scope` are automatically polled regardless of which thread calls it
/// (no explicit ticking required).
///
/// [`Scope::spawn_remote`] submits tasks to a specific remote thread (the second argument
/// of `scope_with`). If no remote thread is specified, it behaves identically to
/// `Scope::spawn_scope`. This API uses a thread executor that cannot be driven across
/// threads; callers must ensure the remote executor is capable of polling tasks on its own.
/// This is typically used to send tasks to the main thread.
/// 
/// ## Examples
/// 
/// ```
/// # use voker_task::{TaskPool, block_on};
/// let task_pool = TaskPool::new();
///
/// let task = task_pool.spawn(async { 1 + 1 });
///
/// assert_eq!(block_on(task), 2);
/// ```
///
/// ```
/// # use voker_task::TaskPool;
/// let task_pool = TaskPool::new();
///
/// let mut results = task_pool.scope(|scope| {
///     for value in 1..=4 {
///         scope.spawn(async move { value * value });
///     }
/// });
///
/// results.sort_unstable();
/// assert_eq!(results, vec![1, 4, 9, 16]);
/// ```
///
/// # Executors
///
/// The design incorporates three distinct executors for different scenarios.
///
/// ## `ThreadExecutor`
///
/// A thread-local executor implemented via `std::thread_local!`.
///
/// Use [`TaskPool::spawn_local`] to submit tasks to this executor. It returns a [`Task`],
/// a thin wrapper around [`async_task::Task`]. [`Task`] behaves like a thread's
/// `JoinHandle`: you can `await` it, [`Task::detach`] it to run in the background,
/// or [`Task::cancel`] it.
///
/// Tasks can be spawned from other threads but will only execute on the owning thread.
/// [`Scope::spawn_scope`] and [`Scope::spawn_remote`] can also submit tasks to this executor.
///
/// **Worker threads**: The `ThreadExecutor` on worker threads runs automatically
/// without explicit ticking.
///
/// **Main thread**: Without active scopes, tasks submitted via `spawn_scope` are **not**
/// automatically polled and require explicit ticking via [`TaskPool::local_ticker`].
/// However, when using [`TaskPool::scope`] (or similar), the executor is automatically driven.
///
/// ## `GlobalExecutor`
///
/// A per‑pool executor (not globally unique) responsible for multi‑threaded scheduling.
///
/// Typically only the main thread holds a `GlobalExecutor`, which contains a thread‑safe
/// task queue. Each worker thread has a `Worker` executor that binds to the `GlobalExecutor`
/// upon thread creation.
///
/// Each `Worker` maintains its own local queue and can steal tasks from the
/// `GlobalExecutor` or from other workers' queues. This implements automatic load‑balanced
/// work distribution.
///
/// Use [`TaskPool::spawn`] or [`Scope::spawn`] to submit tasks to the `GlobalExecutor`,
/// which will wake appropriate threads to execute them.
#[derive(Debug)]
pub struct TaskPool {
    /// The executor for the pool.
    executor: Arc<GlobalExecutor<'static>>,
    /// Worker threads.
    threads: Box<[JoinHandle<()>]>,
    /// Shutdown signal sender.
    shutdown_tx: async_channel::Sender<()>,
}

impl Default for TaskPool {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskPool {
    /// Creates a `TaskPool` with default configuration.
    /// 
    /// The worker count defaults to [`available_parallelism`] and at least `1`.
    /// 
    /// # Examples
    /// 
    /// ```
    /// use voker_task::TaskPool;
    /// let task_pool = TaskPool::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        TaskPoolBuilder::new().build()
    }

    fn new_internal(builder: TaskPoolBuilder) -> Self {
        // shutdown signal
        let (shutdown_tx, shutdown_rx) = async_channel::unbounded::<()>();

        // Set the number of threads based on Builder or available_parallelism.
        let thread_num = builder
            .thread_num
            .unwrap_or_else(|| available_parallelism().get());

        // GlobalExecutor
        let executor = Arc::new(GlobalExecutor::new(thread_num));

        // Create threads
        let threads: Box<[JoinHandle<()>]> = (0..thread_num)
            .map(|i| {
                // clone GlobalExecutor and shutdown signal channel receiver
                let global_ex = Arc::clone(&executor);
                let shutdown_rx = shutdown_rx.clone();

                // Set thread name
                let thread_name = if let Some(thread_name) = &builder.thread_name {
                    format!("{thread_name} ({i})")
                } else {
                    format!("TaskPool ({i})")
                };

                let mut thread_builder = thread::Builder::new().name(thread_name);

                // Set thread stack size
                if let Some(stack_size) = builder.stack_size {
                    thread_builder = thread_builder.stack_size(stack_size);
                }

                let on_thread_spawn = builder.on_thread_spawn.clone();
                let on_thread_destroy = builder.on_thread_destroy.clone();

                thread_builder
                    .spawn(move || {
                        // Move Arc to closure, ensure its validity during thread execution.
                        let global_ex: Arc<GlobalExecutor<'_>> = global_ex;

                        THREAD_EXECUTOR.with(|local_ex| {
                            let local_ex: &ThreadExecutor = local_ex; // From Arc to Inner
                            let ticker: ThreadExecutorTicker = local_ex.ticker().expect("thread-local");
                            global_ex.bind_local_worker(); // bind and initialize `LOCAL_WORKER`.

                            // Call `on_thread_spawn`
                            if let Some(on_spawn) = on_thread_spawn {
                                on_spawn();
                            }

                            // Create a drop guard, call `on_thread_destroy` automatically.
                            let _destructor = CallOnDrop(on_thread_destroy);

                            // Loop working
                            loop {
                                // Future's panic will be propagated to Task, we do not handle here.
                                let res = std::panic::catch_unwind(|| block_on(
                                    global_ex.run(ticker.run(shutdown_rx.recv()))
                                ));
                                // Err -> panicked
                                // Ok(Err(_)) -> shutdown_rx.recv()
                                // Ok(Ok(_)) -> unreachable
                                if let Ok(value) = res {
                                    // Use unwrap_err because we expect a Closed error
                                    value.unwrap_err();
                                    break;
                                }
                            }
                        });
                    })
                    .expect("Failed to spawn thread.")
            })
            .collect();

        Self {
            executor,
            threads,
            shutdown_tx,
        }
    }
    
    /// Returns the number of worker threads in the pool.
    /// 
    /// Does not include the thread where the task pool is located.
    #[inline]
    pub fn thread_num(&self) -> usize {
        self.threads.len()
    }

    /// Returns the thread executor for the current thread.
    #[inline]
    pub fn local_executor() -> Arc<ThreadExecutor<'static>> {
        THREAD_EXECUTOR.with(Clone::clone)
    }

    /// Obtains a `Ticker` that can drive the [`ThreadExecutor`]
    /// on the current thread.
    ///
    /// This is typically used on the main thread to explicitly
    /// poll tasks submitted via [`TaskPool::spawn_local`].
    #[inline]
    pub fn local_ticker() -> ThreadExecutorTicker<'static, 'static> {
        THREAD_EXECUTOR.with(|ex: &Arc<ThreadExecutor>|{
            let ex: &ThreadExecutor = ex; // From Arc to Inner
            #[expect(unsafe_code, reason = "need to transmute lifetime.")]
            let ex: &'static ThreadExecutor = unsafe { mem::transmute(ex) };
            #[expect(unsafe_code, reason = "thread local executor")]
            unsafe { ex.ticker_unchecked() }
        })
    }

    /// Spawns a `'static` but `!Send` future onto the task pool.
    /// 
    /// Because the future is `!Send`, it is submitted to the current thread's
    /// [`ThreadExecutor`].
    /// 
    /// Returns a [`Task`] – a thin wrapper around [`async_task::Task`] – that can
    /// be awaited, canceled or detached.
    /// 
    /// Worker threads automatically tick their `ThreadExecutor`, **but the main
    /// thread does not**. If used on the main thread, you must explicitly tick it
    /// via [`TaskPool::local_ticker`].
    ///
    /// # Example
    ///
    /// ```
    /// use core::cell::Cell;
    /// use std::rc::Rc;
    /// use voker_task::{TaskPool, block_on};
    ///
    /// let pool = TaskPool::new();
    /// let value = Rc::new(Cell::new(0));
    /// let value_for_task = Rc::clone(&value);
    ///
    /// let task = pool.spawn_scope(async move {
    ///     value_for_task.set(7);
    ///     value_for_task.get()
    /// });
    ///
    /// let ticker = TaskPool::local_ticker();
    /// while ticker.try_tick() {}
    ///
    /// assert_eq!(block_on(task), 7);
    /// assert_eq!(value.get(), 7);
    /// ```
    #[inline]
    pub fn spawn_local<T: 'static>(
        &self,
        future: impl Future<Output = T> + 'static,
    ) -> Task<T> {
        #[expect(unsafe_code, reason = "spawn local without Send is safe")]
        THREAD_EXECUTOR.with(|ex| unsafe {
            ex.spawn_unchecked(future)
        })
    }

    /// Spawns a `'static` future onto the task pool.
    ///
    /// The task is submitted to the pool's `GlobalExecutor`, which schedules it
    /// on an appropriate thread.
    ///
    /// Returns a [`Task`] – a thin wrapper around [`async_task::Task`] – that can
    /// be awaited, canceled or detached.
    /// 
    /// The pool will execute the task regardless of whether the user polls the handle.
    ///
    /// - For non‑`Send` futures, use [`TaskPool::spawn_local`].
    /// - For non‑`'static` futures, use [`TaskPool::scope`].
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_task::{TaskPool, block_on};
    ///
    /// let pool = TaskPool::new();
    /// let task = pool.spawn(async { 21 + 21 });
    ///
    /// assert_eq!(block_on(task), 42);
    /// ```
    #[inline]
    pub fn spawn<T: Send + 'static>(
        &self,
        future: impl Future<Output = T> + Send + 'static,
    ) -> Task<T> {
        self.executor.spawn(future)
    }

    /// Allows spawning non‑`'static` futures on the thread pool.
    ///
    /// Takes a callback that receives a scope object, which can be used to spawn
    /// tasks. This function waits for all spawned tasks to complete before
    /// returning.
    ///
    /// Similar to [`thread::scope`] and `rayon::scope`.
    ///
    /// # Example
    ///
    /// ```
    /// use voker_task::TaskPool;
    ///
    /// let pool = TaskPool::new();
    ///
    /// let values = [1_u32, 2, 3, 4];
    ///
    /// let mut results = pool.scope(|scope| {
    ///     for value in &values {
    ///         scope.spawn(async move { *value * 2 });
    ///     }
    /// });
    ///
    /// results.sort_unstable();
    /// assert_eq!(results, vec![2, 4, 6, 8]);
    /// ```
    #[inline]
    pub fn scope<'env, F, T>(&self, f: F) -> Vec<T>
    where
        F: for<'scope> FnOnce(&'scope Scope<'scope, 'env, T>),
        T: Send + 'static,
    {
        THREAD_EXECUTOR.with(|ex| {
            self.scope_with_inner(true, ex, ex, f)
        })
    }

    /// Allows passing an remote executor and controlling whether the global
    /// executor is ticked.
    ///
    /// # Overview
    ///
    /// [`Scope`] provides three spawning methods:
    ///
    /// - [`Scope::spawn`]: submits to the `GlobalExecutor` (work‑stealing, most efficient).
    /// - [`Scope::spawn_scope`]: submits to the current thread's `ThreadExecutor`
    ///   and actively ticks it.
    /// - [`Scope::spawn_remote`]: submits to a specified `ThreadExecutor`.
    ///   If that executor belongs to another thread, the scope still waits for
    ///   completion but cannot actively tick it.
    ///
    /// # Parameters
    ///
    /// - `tick_global`: if `true`, the scope will also tick the global
    ///   executor. This is the default in [`TaskPool::scope`] because `spawn`
    ///   uses the global executor.
    /// - `remote_executor`: the executor used for `spawn_remote`.
    ///   If `None`, the current thread's `ScopeExecutor` is used (as in
    ///   [`TaskPool::scope`]).
    ///
    /// If all your tasks use `spawn_scope`, you can set `tick_global` to `false`;
    /// the scope will then only tick the `ScopeExecutor`, potentially finishing faster.
    #[inline]
    pub fn scope_with<'env, F, T>(
        &self,
        tick_global: bool,
        remote_ex: Option<&ThreadExecutor>,
        f: F,
    ) -> Vec<T>
    where
        F: for<'scope> FnOnce(&'scope Scope<'scope, 'env, T>),
        T: Send + 'static,
    {
        THREAD_EXECUTOR.with(|ex: &Arc<ThreadExecutor>| {
            // If an `remote_executor` is passed, use that.
            // Otherwise, use local executor instead.
            let local: &ThreadExecutor = ex; // From Arc to Inner
            let remote: &ThreadExecutor = remote_ex.unwrap_or(local);
            self.scope_with_inner(tick_global, remote, local, f)
        })
    }

    #[expect(unsafe_code, reason = "need to transmute lifetimes.")]
    fn scope_with_inner<'env, F, T>(
        &self,
        tick_global: bool,
        remote_ex: &ThreadExecutor,
        local_ex: &ThreadExecutor,
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

        // `self.executor` is `Arc<GlobalExecutor>`, not `GlobalExecutor`.
        let global_ex: &GlobalExecutor = &self.executor;
        let global_executor: &'env GlobalExecutor<'env> =
            unsafe { mem::transmute::<&GlobalExecutor, &GlobalExecutor>(global_ex) };
        let remote_executor: &'env ThreadExecutor<'env> =
            unsafe { mem::transmute::<&ThreadExecutor, &ThreadExecutor>(remote_ex) };
        let scope_executor: &'env ThreadExecutor<'env> =
            unsafe { mem::transmute::<&ThreadExecutor, &ThreadExecutor>(local_ex) };

        type FallibleTaskQueue<T> = SegQueue<FallibleTask<Result<T, Box<dyn Any + Send>>>>;
        let task_queue: FallibleTaskQueue<T> = SegQueue::new();
        let spawned_tasks: &'env FallibleTaskQueue<T> =
            unsafe { mem::transmute::<&FallibleTaskQueue<T>, &FallibleTaskQueue<T>>(&task_queue) };

        let scope = Scope {
            global_executor,
            remote_executor,
            scope_executor,
            spawned_tasks,
            scope: PhantomData,
            env: PhantomData,
        };

        let scope_ref: &'env Scope<'_, 'env, T> =
            unsafe { mem::transmute::<&Scope<T>, &Scope<T>>(&scope) };

        f(scope_ref);

        if spawned_tasks.is_empty() {
            // No task, return directly.
            return Vec::new();
        }

        // If there are not worker threads, we must tick global executor explicitly. 
        let tick_global = tick_global || self.threads.is_empty();

        // we get this from a thread local so we should always be on the scope executors thread.
        let local_ticker = unsafe {
            scope_executor.ticker_unchecked()
        };

        // If `local_executor` and `remote_executor` are the same,
        // we should only tick one of them to avoid deadlock.
        //
        // If they differ, `remote_executor` belongs to another thread,
        // so we cannot tick it here.

        let get_results = async {
            let mut results = Vec::with_capacity(spawned_tasks.len());
            while let Some(task) = spawned_tasks.pop() {
                if let Some(ret) = task.await {
                    match ret {
                        Ok(val) => results.push(val),
                        Err(payload) => std::panic::resume_unwind(payload),
                    }
                } else {
                    voker_utils::cold_path();
                    panic!("Failed to catch panic!");
                }
            }
            results
        };

        // block utils all tasks are finished.
        if tick_global {
            block_on(Self::execute_scope_with_global(
                global_executor,
                local_ticker,
                get_results
            ))
        } else {
            block_on(Self::execute_scope(
                local_ticker,
                get_results
            ))
        }
    }

    async fn execute_scope_with_global<'scope, 'ticker, T>(
        global_executor: &'scope GlobalExecutor<'scope>,
        local_ticker: ThreadExecutorTicker<'scope, 'ticker>,
        get_results: impl Future<Output = Vec<T>>,
    ) -> Vec<T> {
        let execute_forever = async {
            loop {
                let tick_forever = async {
                    loop {
                        local_ticker.tick().await;
                    }
                };
                // we don't care if it errors. If a scoped task errors it will propagate to get_results
                let _ok = AssertUnwindSafe(global_executor.run(tick_forever))
                    .catch_unwind().await.is_ok();
            }
        };
        get_results.or(execute_forever).await
    }

    async fn execute_scope<'scope, 'ticker, T>(
        local_ticker: ThreadExecutorTicker<'scope, 'ticker>,
        get_results: impl Future<Output = Vec<T>>,
    ) -> Vec<T> {
        let execute_forever = async {
            loop {
                let tick_forever = async {
                    loop {
                        local_ticker.tick().await;
                    }
                };
                // we don't care if it errors. If a scoped task errors it will propagate to get_results
                let _ok = AssertUnwindSafe(tick_forever)
                    .catch_unwind().await.is_ok();
            }
        };
        get_results.or(execute_forever).await
    }
}

impl Drop for TaskPool {
    fn drop(&mut self) {
        self.shutdown_tx.close();

        let panicking = thread::panicking();

        let threads = mem::take(&mut self.threads);

        for join_handle in threads {
            let res = join_handle.join();
            if !panicking {
                res.expect("Task thread panicked while executing.");
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Scope

/// A [`TaskPool`] scope for running one or more non‑`'static` futures.
///
/// All tasks spawned through a scope are awaited before the enclosing
/// [`TaskPool::scope`] or [`TaskPool::scope_with`] call returns.
#[derive(Debug)]
pub struct Scope<'scope, 'env: 'scope, T> {
    global_executor: &'scope GlobalExecutor<'scope>,
    remote_executor: &'scope ThreadExecutor<'scope>,
    scope_executor: &'scope ThreadExecutor<'scope>,
    spawned_tasks: &'scope SegQueue<FallibleTask<Result<T, Box<dyn Any + Send>>>>,
    // make `Scope` invariant over 'scope and 'env
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
}

const _STATIC_ASSERT_: () = {
    const fn is_send<T: Send>() {}
    const fn is_sync<T: Sync>() {}
    is_send::<Scope<u8>>();
    is_sync::<Scope<u8>>();
};

impl<'scope, 'env, T: Send + 'scope> Scope<'scope, 'env, T> {
    /// Spawns a scoped future onto the task pool.
    ///
    /// Submits the task to the pool's `GlobalExecutor`; it may be executed on
    /// any worker thread.
    ///
    /// The scope must outlive the future. The future's result will be included
    /// in the vector returned by [`TaskPool::scope`].
    ///
    /// For futures that should run on the same thread as the scope, use
    /// [`Scope::spawn_scope`] instead.
    pub fn spawn<Fut: Future<Output = T> + 'scope + Send>(&self, f: Fut) {
        let task = self
            .global_executor
            .spawn(AssertUnwindSafe(f).catch_unwind())
            .fallible();

        self.spawned_tasks.push(task);
    }

    /// Spawns a scoped future onto the thread where the scope is running.
    ///
    /// Submits the task to the current thread's `ThreadExecutor` and actively
    /// ticks it, guaranteeing execution on the current thread.
    ///
    /// The scope must outlive the future. The future's result will be included
    /// in the vector returned by [`TaskPool::scope`].
    ///
    /// Prefer [`Scope::spawn`] unless the future must run on the scope's thread.
    pub fn spawn_scope<Fut: Future<Output = T> + 'scope + Send>(&self, f: Fut) {
        let task = self
            .scope_executor
            .spawn(AssertUnwindSafe(f).catch_unwind())
            .fallible();

        self.spawned_tasks.push(task);
    }

    /// Spawns a scoped future onto the thread of an remote executor.
    ///
    /// Submits the task to the specified `ThreadExecutor`. If that executor
    /// belongs to another thread, the scope cannot actively tick it but still
    /// waits for completion.
    ///
    /// This is typically used to send tasks to the main thread, which should
    /// have additional logic to periodically process tasks from worker threads.
    ///
    /// The scope must outlive the future. The future's result will be included
    /// in the vector returned by [`TaskPool::scope`].
    ///
    /// Prefer [`Scope::spawn`] unless the future must run on the remote thread.
    pub fn spawn_remote<Fut: Future<Output = T> + 'scope + Send>(&self, f: Fut) {
        let task = self
            .remote_executor
            .spawn(AssertUnwindSafe(f).catch_unwind())
            .fallible();

        self.spawned_tasks.push(task);
    }
}

impl<'scope, 'env, T> Drop for Scope<'scope, 'env, T>
where
    T: 'scope,
{
    fn drop(&mut self) {
        block_on(async {
            while let Some(task) = self.spawned_tasks.pop() {
                task.cancel().await;
            }
        });
    }
}

