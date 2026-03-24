use alloc::fmt;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

// -----------------------------------------------------------------------------
// Task

/// Wraps [`async_task::Task`], a spawned task.
///
/// A [`Task`] can be awaited to retrieve the output of its future.
///
/// Dropping a [`Task`] cancels it, which means its future won't be
/// polled again.
/// 
/// - To drop the [`Task`] handle without canceling it, use [`Task::detach()`]
///   instead.
/// - To cancel a task gracefully and wait until it is fully destroyed,
///   use the [`Task::cancel()`] method.
///
/// Note that canceling a task actually wakes it and reschedules one last time.
/// Then, the executor can destroy the task by simply dropping its [`Runnable`]
/// or by invoking [`Runnable::run()`].
/// 
/// [`Runnable`]: async_task::Runnable
/// [`Runnable::run()`]: async_task::Runnable::run
#[must_use = "Tasks are canceled when dropped, use `.detach()` to run them in the background."]
#[repr(transparent)]
pub struct Task<T>(
    pub(super) async_task::Task<T>
);

impl<T> Unpin for Task<T> {}

impl<T> Task<T> {
    /// Detaches the task to let it keep running in the background.
    #[inline(always)]
    pub fn detach(self) {
        self.0.detach();
    }

    /// Cancels the task and waits for it to stop running.
    ///
    /// Returns the task's output if it was completed just before it
    /// got canceled, or [`None`] if it didn't complete.
    ///
    /// While it's possible to simply drop the [`Task`] to cancel it,
    /// this is a cleaner way of canceling because it also waits for
    /// the task to stop running.
    #[inline(always)]
    pub async fn cancel(self) -> Option<T> {
        self.0.cancel().await
    }

    /// Returns `true` if the current task is finished.
    ///
    /// Unlike poll, it doesn't resolve the final value,
    /// it just checks if the task has finished.
    #[inline(always)]
    pub fn is_finished(&self) -> bool {
        self.0.is_finished()
    }
}

impl<T> Future for Task<T> {
    type Output = T;
    #[inline(always)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // `async_task` has `Task` implement `Future`, so we just poll it.
        #[expect(unsafe_code, reason = "project pinned to inner pinned is safe.")]
        unsafe { Pin::new_unchecked(&mut self.0).poll(cx) }
    }
}

// Derive doesn't work for macro types, so we have to implement this manually.
impl<T> fmt::Debug for Task<T> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::Task;

    #[test]
    fn is_sync_send() {
        fn is_sync<T: Sync>() {}
        is_sync::<Task<()>>();

        fn is_send<T: Send>() {}
        is_send::<Task<()>>();
    }
}
