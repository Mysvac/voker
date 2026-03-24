use core::marker::PhantomData;

use async_task::Task;

// -----------------------------------------------------------------------------
// Scope Executor

/// Web stub for API compatibility with multithreaded mode.
///
/// In wasm web mode, `ScopeExecutor` is intentionally unavailable.
/// Use [`crate::TaskPool::scope`] instead.
///
/// # Panics
/// Panics when task-driving methods are called.
#[derive(Debug)]
pub struct ScopeExecutor<'a> {
    _marker: PhantomData<&'a ()>,
}

impl<'task> Default for ScopeExecutor<'task> {
    #[inline(always)]
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<'task> ScopeExecutor<'task> {
    /// Creates a new `ScopeExecutor`
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }

    /// Spawn a task on the thread executor
    ///
    /// # Panics
    ///
    /// Always panics in web mode.
    pub fn spawn<T: Send + 'task>(
        &self,
        _future: impl Future<Output = T> + Send + 'task,
    ) -> Task<T> {
        panic!("`ScopeExecutor` cannot be used in wasm env.")
    }

    /// Gets the [`ScopeExecutorTicker`] for this executor.
    ///
    /// # Panics
    ///
    /// Panic if this function be used in `wasm` env.
    pub fn ticker<'ticker>(&'ticker self) -> Option<ScopeExecutorTicker<'task, 'ticker>> {
        panic!("`ScopeExecutor` cannot be used in wasm env.")
    }

    /// Returns true if `self` and `other`'s executor is same.
    #[inline(always)]
    pub fn is_same(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

// -----------------------------------------------------------------------------
// ScopeExecutorTicker

/// Used to tick the [`ScopeExecutor`].
///
/// The executor does not make progress unless it is
/// manually ticked on the thread it was created on.
///
/// Cannot be used in web mode.
#[derive(Debug)]
pub struct ScopeExecutorTicker<'task, 'ticker> {
    _executor: &'ticker ScopeExecutor<'task>,
    // make type not send or sync
    _marker: PhantomData<*const ()>,
}

impl<'task, 'ticker> ScopeExecutorTicker<'task, 'ticker> {
    /// Tick the thread executor.
    ///
    /// # Panics
    ///
    /// Panic if this function be used in `wasm` env.
    pub async fn tick(&self) {
        panic!("`ScopeExecutor` cannot be used in wasm env.")
    }

    /// Synchronously try to tick a task on the executor.
    ///
    /// Returns false if does not find a task to tick.
    ///
    /// # Panics
    ///
    /// Panic if this function be used in `wasm` env.
    pub fn try_tick(&self) -> bool {
        panic!("`ScopeExecutor` cannot be used in wasm env.")
    }
}
