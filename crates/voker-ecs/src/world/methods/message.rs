use crate::message::{Message, MessageRegistry, Messages};
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

    /// Runs one global message-update pass for all message types in the registry.
    pub fn update_messages(&mut self) {
        MessageRegistry::run_updates(self);
    }
}
