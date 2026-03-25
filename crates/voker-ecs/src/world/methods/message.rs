use crate::message::{Message, MessageRegistry};
use crate::world::World;

impl World {
    /// Registers a message type in the global message registry.
    pub fn register_message<T: Message>(&mut self) {
        self.message_registry.register_message::<T>();
    }

    /// Deregisters a message type from the global message registry.
    pub fn deregister_message<T: Message>(&mut self) {
        self.message_registry.deregister_message::<T>();
    }

    /// Runs one global message-update pass for all message types in the registry.
    pub fn update_messages(&mut self) {
        MessageRegistry::run_updates(self);
    }
}
