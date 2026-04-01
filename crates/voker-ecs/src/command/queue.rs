use core::fmt::Debug;

use voker_os::utils::ListQueue;

use super::CommandObject;

/// A thread-safe FIFO queue of deferred command objects.
///
/// `CommandQueue` is the global sink used by command buffers to submit
/// [`CommandObject`] instances for later execution.
pub struct CommandQueue {
    queue: ListQueue<CommandObject>,
}

impl Debug for CommandQueue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CommmandQueue").field("len", &self.queue.len()).finish()
    }
}

impl CommandQueue {
    /// Creates an empty command queue.
    pub(crate) fn new() -> Self {
        Self {
            queue: ListQueue::default(),
        }
    }

    /// Returns the number of queued commands.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Returns `true` if the queue contains no commands.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Pops and returns the next command from the front of the queue.
    ///
    /// Returns `None` if the queue is empty.
    ///
    /// Unlike `push`, `pop` has no batched optimized variant.
    ///
    /// In typical usage, popping commands happens while holding a mutable
    /// borrow of `World` (exclusive access). In that context, cache-line
    /// invalidation is not a concern, so pre-locking is unnecessary.
    #[must_use]
    pub fn pop(&self) -> Option<CommandObject> {
        self.queue.pop()
    }

    /// Pushes a command to the back of the queue.
    pub fn push(&self, command: CommandObject) {
        self.queue.push(command);
    }

    /// Extends the queue by appending all commands from an iterator.
    ///
    /// This method acquires the queue's push lock once and reuses it for all
    /// inserted commands to reduce synchronization overhead.
    pub fn extend(&self, iter: impl IntoIterator<Item = CommandObject>) {
        let iter = iter.into_iter();
        let mut guard = self.queue.lock_push();
        iter.for_each(|command| {
            self.queue.push_with_lock(&mut guard, command);
        });
    }
}
