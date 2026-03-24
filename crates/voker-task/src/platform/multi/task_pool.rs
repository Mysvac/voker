use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box;
use alloc::format;
use core::any::Any;
use core::future::Future;
use core::marker::PhantomData;
use core::mem;
use core::panic::AssertUnwindSafe;

use std::thread;
use std::thread_local;
use std::thread::JoinHandle;

use voker_os::sync::Arc;
use futures_lite::FutureExt;
use voker_os::utils::ListQueue;
use async_task::FallibleTask;

use super::GlobalExecutor;
use super::LocalExecutor;
use super::{ScopeExecutor, ScopeExecutorTicker};
use super::{block_on, Task};

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
///     .thread_name("doc".to_string())
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
    thread_name: Option<String>,
    /// Called on thread spawn.
    on_thread_spawn: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
    /// Called on thread termination.
    on_thread_destroy: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
}

impl TaskPoolBuilder {
    /// Creates a new [`TaskPoolBuilder`].
    #[inline(always)]
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
    pub fn thread_name(mut self, thread_name: String) -> Self {
        self.thread_name = Some(thread_name);
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
    pub fn build(self) -> TaskPool {
        TaskPool::new_internal(self)
    }
}

// -----------------------------------------------------------------------------
// TaskPool

thread_local! {
    static LOCAL_EXECUTOR: LocalExecutor<'static> = const { LocalExecutor::new() };
    static SCOPE_EXECUTOR: Arc<ScopeExecutor<'static>> = Arc::new(ScopeExecutor::new());
}

/// A thread pool for executing tasks.
///
/// Manages multithreaded resources and schedules/executes asynchronous tasks.
///
/// ---
///
/// # Functions
///
/// The pool provides four primary interfaces:
///
/// - [`TaskPool::spawn`]
/// - [`TaskPool::spawn_local`]
/// - [`TaskPool::scope`]
/// - [`TaskPool::scope_with_executor`]
///
/// The `spawn` family only accepts `'static` tasks, while `scope` can handle
/// non‑`'static` tasks.
///
/// Specifically:
/// - `spawn_local` accepts non‑`Send` tasks.
/// - `scope_with_executor` allows sending tasks to a specific thread.
///
/// Returned task handles are futures themselves, so they can be awaited with
/// [`block_on`] or composed with other async utilities.
/// 
/// ## Examples
///
/// ```
/// use voker_task::TaskPool;
/// let task_pool = TaskPool::new();
///
/// let mut results = task_pool.scope(|scope| {
///     for value in [1_u32, 2, 3, 4] {
///         scope.spawn(async move { value * value });
///     }
/// });
///
/// results.sort_unstable();
/// assert_eq!(results, vec![1, 4, 9, 16]);
/// ```
/// 
/// ---
/// 
/// # Executors
/// 
/// We designed three different executors for different scenarios.
///
/// ## `LocalExecutor`
///
/// A thread‑local executor implemented via `std::thread_local!`. It is designed
/// for tasks that cannot be sent across threads (`!Send`).
///
/// Use [`TaskPool::spawn_local`] to submit tasks to this executor. It returns a
/// [`Task`] – a thin wrapper around [`async_task::Task`].
///
/// [`Task`] resembles a thread’s `JoinHandle`: you can `await` it, call
/// [`Task::detach`] to let it run in the background, or [`Task::cancel`] to
/// cancel it.
///
/// ## `ScopeExecutor`
///
/// Similar to `LocalExecutor`, it uses thread‑local storage.
///
/// It allows spawning tasks from other threads, but tasks only execute on the
/// owning thread. Because of this, `ScopeExecutor` does not automatically
/// execute tasks; it requires explicit ticking.
///
/// Use [`TaskPool::scope`] and [`Scope::spawn_on_scope`] to submit tasks to
/// this executor. In this case it behaves like `LocalExecutor` – tasks run on
/// the current thread and the [`Scope`] drives them.
///
/// Additionally, it can be used for cross‑thread task transfer. With
/// [`TaskPool::scope_with_executor`] and [`Scope::spawn_on_external`], tasks can
/// be sent to a designated thread (typically the main thread, which must have
/// additional logic to process incoming tasks).
///
/// ## `GlobalExecutor`
///
/// A per‑pool executor (not globally unique) responsible for multithreaded
/// scheduling.
///
/// Usually only the main thread holds a `GlobalExecutor`, which contains a
/// thread‑safe task queue. Each worker thread has a `Worker` executor that binds
/// to the `GlobalExecutor` when the thread is created.
///
/// Each `Worker` has a local queue and can steal tasks from the `GlobalExecutor`
/// or from other `Worker`s, executing them on its own thread. This implements
/// automatic load‑balanced distribution.
///
/// Use [`TaskPool::spawn`] or [`Scope::spawn`] to submit tasks to the
/// `GlobalExecutor`, which will wake appropriate threads to execute them.
///
/// ## Choosing the Right Interface
///
/// - For `'static` tasks: use [`TaskPool::spawn`].
/// - For non‑`'static` tasks: use [`TaskPool::scope`].
///
/// In general, if your task is `Send`, prefer [`TaskPool::spawn`] or
/// [`Scope::spawn`]; they use the `GlobalExecutor` and benefit from
/// multithreaded load balancing.
///
/// If your task is `!Send`, you must use [`TaskPool::spawn_local`].
///
/// To restrict a non‑`'static` task to the current thread, use
/// [`Scope::spawn_on_scope`]. To send a task to another thread (e.g., to the
/// main thread), use [`Scope::spawn_on_external`].
///
/// Note that executors are primarily schedulers; their performance overhead
/// comes from task distribution, not from the tasks themselves.
///
/// - `LocalExecutor` is entirely single‑threaded with no synchronization.
/// - `GlobalExecutor` involves synchronization for work‑stealing, but this
///   overhead is independent of the actual task work.
#[derive(Debug)]
pub struct TaskPool {
    /// The executor for the pool.
    executor: Arc<GlobalExecutor<'static>>,
    /// Worker threads.
    threads: Box<[JoinHandle<()>]>,
    /// Shutdown signal sender.
    shutdown_tx: async_channel::Sender<()>,
}

impl TaskPool {
    /// Creates a `TaskPool` with default configuration.
    /// 
    /// The worker count defaults to [`std::thread::available_parallelism`] and is
    /// never lower than `1`.
    pub fn new() -> Self {
        TaskPoolBuilder::new().build()
    }

    fn new_internal(builder: TaskPoolBuilder) -> Self {
        // shutdown signal
        let (shutdown_tx, shutdown_rx) = async_channel::unbounded::<()>();

        // Set the number of threads based on Builder or available_parallelism.
        let thread_num = builder
            .thread_num
            .unwrap_or(voker_os::thread::available_parallelism().get());

        // GlobalExecutor
        let executor = Arc::new(GlobalExecutor::new(thread_num));

        // Create threads
        let threads: Box<[JoinHandle<()>]> = (0..thread_num)
            .map(|i| {
                // clone GlobalExecutor and shutdown signal channel receiver
                let global_ex = Arc::clone(&executor);
                let shutdown_rx = shutdown_rx.clone();

                // Set thread name
                let thread_name = if let Some(thread_name) = builder.thread_name.as_deref() {
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
                        // Ensure its validity during thread execution.
                        let global_ex: Arc<GlobalExecutor<'_>> = global_ex;
                        // bind and initialize `LOCAL_WORKER`.
                        global_ex.bind_local_worker();

                        LOCAL_EXECUTOR.with(|local_ex| {
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
                                    global_ex.run(local_ex.run(shutdown_rx.recv()))
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

    /// Runs a function with the local executor.
    ///
    /// Typically used to tick the local executor on the main thread
    /// when it must share time with other work.
    ///
    /// This is mainly relevant for tasks spawned through
    /// [`TaskPool::spawn_local`] from the main thread.
    /// 
    /// The local executor of the worker thread will be automatically
    /// executed without needing manual ticking.
    #[inline]
    pub fn with_local_executor<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&LocalExecutor) -> R,
    {
        LOCAL_EXECUTOR.with(f)
    }

    /// Returns the scope executor for the current thread.
    ///
    /// Each thread should create only one `ScopeExecutor`;
    /// otherwise deadlocks may occur.
    /// 
    /// Usually this is obtained on the main thread and passed to workers so they
    /// can submit thread-affine tasks back to that thread via
    /// [`TaskPool::scope_with_executor`].
    #[inline]
    pub fn get_scope_executor() -> Arc<ScopeExecutor<'static>> {
        SCOPE_EXECUTOR.with(Clone::clone)
    }

    /// Spawns a `'static` future onto the task pool.
    ///
    /// The task is submitted to the pool's `GlobalExecutor`, which schedules it
    /// on an appropriate thread.
    ///
    /// Returns a [`Task`] – a thin wrapper around [`async_task::Task`] – that can
    /// be awaited for the result.
    ///
    /// The task can be canceled or detached (allowing it to continue even if the
    /// handle is dropped). The pool will execute the task regardless of whether
    /// the user polls the handle.
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
        Task(self.executor.spawn(future))
    }

    /// Spawns a `'static` but `!Send` future onto the task pool.
    ///
    /// Because the future is `!Send`, it is submitted to the current thread's
    /// `LocalExecutor`.
    /// 
    /// Worker threads automatically tick their `LocalExecutor`, **but the main
    /// thread does not**. If used on the main thread, you must explicitly tick it
    /// via [`with_local_executor`].
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
    /// let task = pool.spawn_local(async move {
    ///     value_for_task.set(7);
    ///     value_for_task.get()
    /// });
    ///
    /// pool.with_local_executor(|executor| {
    ///     while executor.try_tick() {}
    /// });
    ///
    /// assert_eq!(block_on(task), 7);
    /// assert_eq!(value.get(), 7);
    /// ```
    ///
    /// [`with_local_executor`]: Self::with_local_executor
    #[inline]
    pub fn spawn_local<T: 'static>(
        &self,
        future: impl Future<Output = T> + 'static,
    ) -> Task<T> {
        Task(LOCAL_EXECUTOR.with(|ex| ex.spawn(future)))
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
        SCOPE_EXECUTOR.with(|scope_executor| {
            self.scope_with_executor_inner(true, scope_executor, scope_executor, f)
        })
    }

    /// Allows passing an external executor and controlling whether the global
    /// executor is ticked.
    ///
    /// # Overview
    ///
    /// [`Scope`] provides three spawning methods:
    ///
    /// - [`Scope::spawn`]: submits to the `GlobalExecutor` (work‑stealing, most efficient).
    /// - [`Scope::spawn_on_scope`]: submits to the current thread's `ScopeExecutor`
    ///   and actively ticks it.
    /// - [`Scope::spawn_on_external`]: submits to a specified `ScopeExecutor`.
    ///   If that executor belongs to another thread, the scope still waits for
    ///   completion but cannot actively tick it.
    ///
    /// # Parameters
    ///
    /// - `tick_global_executor`: if `true`, the scope will also tick the global
    ///   executor. This is the default in [`TaskPool::scope`] because `spawn`
    ///   uses the global executor.
    /// - `external_executor`: the executor used for [`spawn_on_external`].
    ///   If `None`, the current thread's `ScopeExecutor` is used (as in
    ///   [`TaskPool::scope`]).
    ///
    /// If all your tasks use `spawn_on_scope`, you can set `tick_global_executor`
    /// to `false`; the scope will then only tick the `ScopeExecutor`, potentially
    /// finishing faster.
    ///
    /// [`Scope::spawn`]: crate::Scope::spawn
    /// [`Scope::spawn_on_scope`]: crate::Scope::spawn_on_scope
    /// [`Scope::spawn_on_external`]: crate::Scope::spawn_on_external
    /// [`spawn_on_external`]: crate::Scope::spawn_on_external
    pub fn scope_with_executor<'env, F, T>(
        &self,
        tick_global_executor: bool,
        external_executor: Option<&ScopeExecutor>,
        f: F,
    ) -> Vec<T>
    where
        F: for<'scope> FnOnce(&'scope Scope<'scope, 'env, T>),
        T: Send + 'static,
    {
        SCOPE_EXECUTOR.with(|scope_executor| {
            // If an `external_executor` is passed, use that. Otherwise, get the executor stored
            // in the `THREAD_EXECUTOR` thread local.

            if let Some(external_executor) = external_executor {
                self.scope_with_executor_inner(
                    tick_global_executor,
                    external_executor,
                    scope_executor,
                    f,
                )
            } else {
                self.scope_with_executor_inner(
                    tick_global_executor,
                    scope_executor,
                    scope_executor,
                    f,
                )
            }
        })
    }

    #[expect(unsafe_code, reason = "Required to transmute lifetimes.")]
    fn scope_with_executor_inner<'env, F, T>(
        &self,
        tick_global_executor: bool,
        external_executor: &ScopeExecutor,
        scope_executor: &ScopeExecutor,
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
        let global_executor: &GlobalExecutor = &self.executor;
        let global_executor: &'env GlobalExecutor<'env> =
            unsafe { mem::transmute::<&GlobalExecutor, &GlobalExecutor>(global_executor) };
        let external_executor: &'env ScopeExecutor<'env> =
            unsafe { mem::transmute::<&ScopeExecutor, &ScopeExecutor>(external_executor) };
        let scope_executor: &'env ScopeExecutor<'env> =
            unsafe { mem::transmute::<&ScopeExecutor, &ScopeExecutor>(scope_executor) };

        type FallibleTaskQueue<T> = ListQueue<FallibleTask<Result<T, Box<dyn Any + Send>>>>;
        let task_queue: FallibleTaskQueue<T> = ListQueue::default();
        let spawned: &'env FallibleTaskQueue<T> =
            unsafe { mem::transmute::<&FallibleTaskQueue<T>, &FallibleTaskQueue<T>>(&task_queue) };

        let scope = Scope {
            global_executor,
            external_executor,
            scope_executor,
            spawned,
            scope: PhantomData,
            env: PhantomData,
        };

        let scope_ref: &'env Scope<'_, 'env, T> =
            unsafe { mem::transmute::<&Scope<T>, &Scope<T>>(&scope) };

        f(scope_ref);

        if spawned.is_empty() {
            // No task, return directly.
            return Vec::new();
        }

        // block utils all tasks are finished.
        block_on(async move {
            let get_results = async {
                let mut results = Vec::with_capacity(spawned.len());
                while let Some(task) = spawned.pop() {
                    if let Some(res) = task.await {
                        match res {
                            Ok(res) => results.push(res),
                            Err(payload) => std::panic::resume_unwind(payload),
                        }
                    } else {
                        panic!("Failed to catch panic!");
                    }
                }
                results
            };

            let tick_global_executor = tick_global_executor || self.threads.is_empty();

            // we get this from a thread local so we should always be on the scope executors thread.
            let scope_ticker = scope_executor.ticker().unwrap();

            // If `scope_executor` and `external_executor` are the same,
            // we should only tick one of them to avoid deadlock.
            //
            // If they differ, `external_executor` belongs to another thread,
            // so we cannot tick it here.

            if tick_global_executor {
                Self::execute_global_scope(
                    global_executor,
                    scope_ticker,
                    get_results
                ).await
            } else {
                Self::execute_scope(
                    scope_ticker,
                    get_results
                ).await
            }
        })
    }

    #[inline]
    async fn execute_global_scope<'scope, 'ticker, T>(
        global_executor: &'scope GlobalExecutor<'scope>,
        scope_ticker: ScopeExecutorTicker<'scope, 'ticker>,
        get_results: impl Future<Output = Vec<T>>,
    ) -> Vec<T> {
        let execute_forever = async {
            loop {
                let tick_forever = async {
                    loop {
                        scope_ticker.tick().await;
                    }
                };
                // we don't care if it errors. If a scoped task errors it will propagate to get_results
                let _ok = AssertUnwindSafe(global_executor.run(tick_forever))
                    .catch_unwind()
                    .await
                    .is_ok();
            }
        };
        get_results.or(execute_forever).await
    }

    #[inline]
    async fn execute_scope<'scope, 'ticker, T>(
        scope_ticker: ScopeExecutorTicker<'scope, 'ticker>,
        get_results: impl Future<Output = Vec<T>>,
    ) -> Vec<T> {
        let execute_forever = async {
            loop {
                let tick_forever = async {
                    loop {
                        scope_ticker.tick().await;
                    }
                };
                // we don't care if it errors. If a scoped task errors it will propagate to get_results
                let _ok = AssertUnwindSafe(tick_forever).catch_unwind().await.is_ok();
            }
        };
        get_results.or(execute_forever).await
    }
}

impl Default for TaskPool {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TaskPool {
    fn drop(&mut self) {
        self.shutdown_tx.close();

        let panicking = thread::panicking();

        let threads = mem::replace(&mut self.threads, Box::new([]));

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
/// [`TaskPool::scope`] or [`TaskPool::scope_with_executor`] call returns.
#[derive(Debug)]
pub struct Scope<'scope, 'env: 'scope, T> {
    global_executor: &'scope GlobalExecutor<'scope>,
    external_executor: &'scope ScopeExecutor<'scope>,
    scope_executor: &'scope ScopeExecutor<'scope>,
    spawned: &'scope ListQueue<FallibleTask<Result<T, Box<dyn Any + Send>>>>,
    // make `Scope` invariant over 'scope and 'env
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
}

const _STATIC_ASSERT_: () = {
    const fn is_send<T: Send>() {}
    const fn is_sync<T: Sync>() {}
    is_send::<Scope<()>>();
    is_sync::<Scope<()>>();
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
    /// [`Scope::spawn_on_scope`] instead.
    pub fn spawn<Fut: Future<Output = T> + 'scope + Send>(&self, f: Fut) {
        let task = self
            .global_executor
            .spawn(AssertUnwindSafe(f).catch_unwind())
            .fallible();

        self.spawned.push(task);
    }

    /// Spawns a scoped future onto the thread where the scope is running.
    ///
    /// Submits the task to the current thread's `ScopeExecutor` and actively
    /// ticks it, guaranteeing execution on the current thread.
    ///
    /// The scope must outlive the future. The future's result will be included
    /// in the vector returned by [`TaskPool::scope`].
    ///
    /// Prefer [`Scope::spawn`] unless the future must run on the scope's thread.
    pub fn spawn_on_scope<Fut: Future<Output = T> + 'scope + Send>(&self, f: Fut) {
        let task = self
            .scope_executor
            .spawn(AssertUnwindSafe(f).catch_unwind())
            .fallible();

        self.spawned.push(task);
    }

    /// Spawns a scoped future onto the thread of an external executor.
    ///
    /// Submits the task to the specified `ScopeExecutor`. If that executor
    /// belongs to another thread, the scope cannot actively tick it but still
    /// waits for completion.
    ///
    /// This is typically used to send tasks to the main thread, which should
    /// have additional logic to periodically process tasks from worker threads.
    ///
    /// The scope must outlive the future. The future's result will be included
    /// in the vector returned by [`TaskPool::scope`].
    ///
    /// Prefer [`Scope::spawn`] unless the future must run on the external thread.
    pub fn spawn_on_external<Fut: Future<Output = T> + 'scope + Send>(&self, f: Fut) {
        let task = self
            .external_executor
            .spawn(AssertUnwindSafe(f).catch_unwind())
            .fallible();

        self.spawned.push(task);
    }
}

impl<'scope, 'env, T> Drop for Scope<'scope, 'env, T>
where
    T: 'scope,
{
    fn drop(&mut self) {
        block_on(async {
            while let Some(task) = self.spawned.pop() {
                task.cancel().await;
            }
        });
    }
}

