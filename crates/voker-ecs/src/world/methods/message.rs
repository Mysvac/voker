use core::any::TypeId;

use crate::message::{Message, MessageId, MessageKey};
use crate::message::{MessageKeyIter, MessageQueue, Messages};
use crate::utils::DebugName;
use crate::world::World;

impl World {
    /// Registers a message type in the global message registry.
    pub fn register_message<M: Message>(&mut self) -> MessageId {
        self.init_resource::<MessageQueue<M>>();
        self.messages.register::<M>(&mut self.resources)
    }

    /// Looks up a message ID by its TypeId.
    pub fn get_message_id<M: Message>(&self) -> Option<MessageId> {
        self.messages.get_id(TypeId::of::<M>())
    }

    /// Writes a [`Message`].
    ///
    /// This method returns the [`MessageKey`] of the written `message`,
    /// or [`None`] if the `message` could not be written.
    ///
    /// # Panics
    /// Panics if the [`Message`] is unregistered.
    pub fn write_message<M: Message>(&mut self, message: M) -> Option<MessageKey<M>> {
        let Some(mut msgs) = self.get_resource_mut::<MessageQueue<M>>() else {
            unregistered_message(DebugName::type_name::<M>());
            return None;
        };
        Some(msgs.write(message))
    }

    /// Writes a batch of [`Message`]s from an iterator.
    ///
    /// This method returns the [IDs](`MessageKey`) of the written `messages`,
    /// or [`None`] if the `events` could not be written.
    ///
    /// # Panics
    /// Panics if the [`Message`] is unregistered.
    pub fn write_message_batch<M: Message>(
        &mut self,
        messages: impl IntoIterator<Item = M>,
    ) -> Option<MessageKeyIter<M>> {
        let Some(mut msgs) = self.get_resource_mut::<MessageQueue<M>>() else {
            unregistered_message(DebugName::type_name::<M>());
            return None;
        };
        Some(msgs.write_batch(messages))
    }

    /// Updates all registered message queues.
    ///
    /// This runs the per-queue maintenance pass used by the message subsystem,
    /// including internal buffer rotation and cleanup of expired message data.
    ///
    /// In normal app flow, this is invoked automatically by `App`'s
    /// `MainSchedulePlugin`. Users should generally not call this manually.
    /// Calling it at the wrong time can advance/clear queue state early and may
    /// cause readers to miss messages.
    pub fn update_messages(this: &mut Self) {
        Messages::run_updates(this);
    }
}

#[cold]
#[inline(never)]
fn unregistered_message(name: DebugName) {
    log::error!(
        "Unable to write message `{name}`, call `World::register_message` before write it."
    );
}
