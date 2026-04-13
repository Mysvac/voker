use crate::message::{Message, MessageId, MessageIdIter, Messages};
use crate::utils::DebugName;
use crate::world::World;

impl World {
    /// Registers a message type in the global message registry.
    pub fn register_message<T: Message>(&mut self) {
        self.message_registry.register_message::<T>();
        self.init_resource::<Messages<T>>();
    }

    /// Deregisters a message type from the global message registry.
    pub fn unregister_message<T: Message>(&mut self) {
        self.message_registry.unregister_message::<T>();
        self.drop_resource::<Messages<T>>();
    }

    /// Writes a [`Message`].
    ///
    /// This method returns the [`MessageId`] of the written `message`,
    /// or [`None`] if the `message` could not be written.
    pub fn write_message<M: Message>(&mut self, message: M) -> Option<MessageId<M>> {
        let Some(mut msgs) = self.get_resource_mut::<Messages<M>>() else {
            unregistered_message(DebugName::type_name::<M>());
            return None;
        };
        Some(msgs.write(message))
    }

    /// Writes a batch of [`Message`]s from an iterator.
    ///
    /// This method returns the [IDs](`MessageId`) of the written `messages`,
    /// or [`None`] if the `events` could not be written.
    pub fn write_message_batch<M: Message>(
        &mut self,
        messages: impl IntoIterator<Item = M>,
    ) -> Option<MessageIdIter<M>> {
        let Some(mut msgs) = self.get_resource_mut::<Messages<M>>() else {
            unregistered_message(DebugName::type_name::<M>());
            return None;
        };
        Some(msgs.write_batch(messages))
    }
}

#[cold]
#[inline(never)]
fn unregistered_message(name: DebugName) {
    log::error!(
        "Unable to write message `{name}`, call `World::register_message` before write it."
    );
}
