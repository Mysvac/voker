
use alloc::string::String;
use alloc::vec::Vec;

use core::cell::{RefCell, Cell};
use core::future::Future;
use core::marker::PhantomData;
use core::mem;

use voker_os::sync::Arc;

use super::LocalExecutor;
use super::ScopeExecutor;
use super::{block_on, Task};

// -----------------------------------------------------------------------------
// TaskPoolBuilder

/// Used to create a [`TaskPool`].
#[derive(Default)]
pub struct TaskPoolBuilder {}

impl TaskPoolBuilder {
    /// Creates a new `TaskPoolBuilder` instance
    #[inline(always)]
    pub const fn new() -> Self {
        Self{}
    }

    /// No op on the single threaded task pool
    #[inline(always)]
    pub fn thread_num(self, _thread_num: usize) -> Self {
        self
    }

    /// No op on the single threaded task pool
    #[inline(always)]
    pub fn stack_size(self, _stack_size: usize) -> Self {
        self
    }

    /// No op on the single threaded task pool
    #[inline(always)]
    pub fn thread_name(self, _thread_name: String) -> Self {
        self
    }

    /// No op on the single threaded task pool
    #[inline(always)]
    pub fn on_thread_spawn(self, _f: impl Fn() + Send + Sync + 'static) -> Self {
        self
    }

    /// No op on the single threaded task pool
    #[inline(always)]
    pub fn on_thread_destroy(self, _f: impl Fn() + Send + Sync + 'static) -> Self {
        self
    }

    /// Creates a new [`TaskPool`]
    #[inline(always)]
    pub fn build(self) -> TaskPool {
        TaskPool {}
    }
}

// -----------------------------------------------------------------------------
// Static Executor

std::thread_local! {
    static LOCAL_EXECUTOR: LocalExecutor<'static> = const { LocalExecutor::new() };
}

// -----------------------------------------------------------------------------
// TaskPool

/// A thread pool for executing tasks.
/// 
/// Tasks are futures that are being automatically driven by the pool
/// on threads owned by the pool. In this case - main thread only.
#[derive(Debug, Default)]
pub struct TaskPool {}

impl TaskPool {
    /// Create a `TaskPool` with the default configuration.
    #[inline(always)]
    pub fn new() -> Self {
        TaskPool {}
    }

    /// Return the number of threads owned by the task pool
    ///
    /// Always return `1` in no_std env.
    #[inline(always)]
    pub fn thread_num(&self) -> usize {
        1
    }

    /// Runs a function with the local executor.
    /// 
    /// Typically used to tick the local executor on the
    /// main thread as it needs to share time with other things.
    ///
    /// ```ignore
    /// use voker_task::TaskPool;
    ///
    /// TaskPool::new().with_local_executor(|local_executor| {
    ///     local_executor.try_tick();
    /// });
    /// ```
    pub fn with_local_executor<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&LocalExecutor) -> R,
    {
        LOCAL_EXECUTOR.with(f)
    }

    /// Just create a new `ScopeExecutor` for wasm
    #[inline]
    pub fn get_thread_executor() -> Arc<ScopeExecutor<'static>> {
        Arc::new(ScopeExecutor::new())
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
        F: for<'scope> FnOnce(&'scope mut Scope<'scope, 'env, T>),
        T: Send + 'static,
    {
        self.scope_with_executor(false, None, f)
    }

    /// Allows spawning non-`'static` futures on the thread pool.
    /// 
    /// The function takes a callback, passing a scope object into it.
    /// The scope object provided to the callback can be used to spawn
    /// tasks. This function will await the completion of all tasks before
    /// returning.
    ///
    /// This is similar to `rayon::scope` and `crossbeam::scope`
    #[expect(unsafe_code, reason = "Required to transmute lifetimes.")]
    pub fn scope_with_executor<'env, F, T>(
        &self,
        _tick_task_pool_executor: bool,
        _thread_executor: Option<&ScopeExecutor>,
        f: F,
    ) -> Vec<T>
    where
        F: for<'scope> FnOnce(&'scope mut Scope<'scope, 'env, T>),
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

        let executor = LocalExecutor::new();
        // SAFETY: As above, all futures must complete in this function so we can change the lifetime
        let executor_ref: &'env LocalExecutor<'env> =
            unsafe { mem::transmute::<&LocalExecutor, &LocalExecutor>(&executor) };

        let results: RefCell<Vec<Option<T>>> = RefCell::new(Vec::new());
        // SAFETY: As above, all futures must complete in this function so we can change the lifetime
        let results_ref: &'env RefCell<Vec<Option<T>>> =
            unsafe { mem::transmute::<&RefCell<Vec<Option<T>>>, &RefCell<Vec<Option<T>>>>(&results) };

        let pending_tasks: Cell<usize> = Cell::new(0);
        // SAFETY: As above, all futures must complete in this function so we can change the lifetime
        let pending_tasks: &'env Cell<usize> =
            unsafe { mem::transmute::<&Cell<usize>, &Cell<usize>>(&pending_tasks) };

        let mut scope = Scope {
            executor_ref,
            pending_tasks,
            results_ref,
            scope: PhantomData,
            env: PhantomData,
        };

        // SAFETY: As above, all futures must complete in this function so we can change the lifetime
        let scope_ref: &'env mut Scope<'_, 'env, T> =
            unsafe { mem::transmute::<&Scope<T>, &Scope<T>>(&mut scope) };

        f(scope_ref);

        // Wait until the scope is complete
        block_on(executor.run(async {
            while pending_tasks.get() != 0 {
                futures_lite::future::yield_now().await;
            }
        }));

        results
            .take()
            .into_iter()
            .map(|result| result.unwrap())
            .collect()
    }

    /// Spawns a static future onto the thread pool.
    /// 
    /// The returned Task is a future, which can be polled to
    /// retrieve the output of the original future.
    /// 
    /// Dropping the task will attempt to cancel it. It can also be
    /// "detached", allowing it to continue running without having
    /// to be polled by the end-user.
    ///
    /// If the provided future is non-`Send`, [`TaskPool::spawn_local`]
    /// should be used instead.
    pub fn spawn<T>(
        &self,
        future: impl Future<Output = T> + 'static/* + Send */,
    ) -> Task<T> 
    where
        T: 'static/* + Send */
    {
        Task::wrap_future(future)
    }

    /// Spawns a static future on the JS event loop.
    /// 
    /// This is exactly the same as [`TaskPool::spawn`].
    pub fn spawn_local<T: 'static>(
        &self,
        future: impl Future<Output = T> + 'static,
    ) -> Task<T> {
        Task::wrap_future(future)
    }
}

// -----------------------------------------------------------------------------
// Scope

/// A `TaskPool` scope for running one or more non-`'static` futures.
///
/// For more information, see [`TaskPool::scope`].
#[derive(Debug)]
pub struct Scope<'scope, 'env: 'scope, T> {
    executor_ref: &'scope LocalExecutor<'scope>,
    // The number of pending tasks spawned on the scope
    pending_tasks: &'scope Cell<usize>,
    // Vector to gather results of all futures spawned during scope run
    results_ref: &'env RefCell<Vec<Option<T>>>,

    // make `Scope` invariant over 'scope and 'env
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
}

#[expect(unsafe_code, reason = "Assuming it's single threaded environment.")]
unsafe impl<T: Send> Send for Scope<'_, '_, T> {}
#[expect(unsafe_code, reason = "Assuming it's single threaded environment.")]
unsafe impl<T: Send> Sync for Scope<'_, '_, T> {}

const _STATIC_ASSERT_: () = {
    const fn is_send<T: Send>() {}
    const fn is_sync<T: Sync>() {}
    is_send::<Scope<()>>();
    is_sync::<Scope<()>>();
};

impl<'scope, 'env, T: Send + 'env> Scope<'scope, 'env, T> {
    /// Spawns a scoped future onto the executor.
    /// 
    /// The scope *must* outlive the provided future. The results of the future
    /// will be returned as a part of [`TaskPool::scope`]'s return value.
    ///
    /// On the single threaded task pool, it just calls [`Scope::spawn_on_scope`].
    ///
    /// For more information, see [`TaskPool::scope`].
    pub fn spawn<Fut: Future<Output = T> + 'scope>(&self, f: Fut) {
        self.spawn_on_scope(f);
    }

    /// Spawns a scoped future onto the executor.
    /// 
    /// The scope *must* outlive the provided future. The results of the future
    /// will be returned as a part of [`TaskPool::scope`]'s return value.
    ///
    /// On the single threaded task pool, it just calls [`Scope::spawn_on_scope`].
    ///
    /// For more information, see [`TaskPool::scope`].
    pub fn spawn_on_external<Fut: Future<Output = T> + 'scope>(&self, f: Fut) {
        self.spawn_on_scope(f);
    }

    /// Spawns a scoped future onto the executor.
    /// 
    /// The scope *must* outlive the provided future. The results of the future
    /// will be returned as a part of [`TaskPool::scope`]'s return value.
    ///
    /// For more information, see [`TaskPool::scope`].
    pub fn spawn_on_scope<Fut: Future<Output = T> + 'scope>(&self, f: Fut) {
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

        // spawn the job itself
        self.executor_ref.spawn(f).detach();
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(all(test, feature = "web"))]
mod test {
    use std::{time, thread};

    use super::*;

    /// This test creates a scope with a single task that goes to sleep for a
    /// nontrivial amount of time. At one point, the scope would (incorrectly)
    /// return early under these conditions, causing a crash.
    ///
    /// The correct behavior is for the scope to block until the receiver is
    /// woken by the external thread.
    #[test]
    fn scoped_spawn() {
        let (sender, receiver) = async_channel::unbounded();
        let task_pool = TaskPool {};
        let _thread = thread::spawn(move || {
            let duration = time::Duration::from_millis(50);
            thread::sleep(duration);
            let _ = sender.send(0);
        });
        task_pool.scope(|scope| {
            scope.spawn(async {
                receiver.recv().await
            });
        });
    }
}
