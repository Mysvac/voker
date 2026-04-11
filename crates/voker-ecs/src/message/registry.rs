use alloc::vec::Vec;
use core::any::TypeId;
use core::fmt::Debug;

use voker_utils::extra::TypeIdMap;

use super::{Message, Messages};
use crate::resource::Resource;
use crate::utils::DebugName;
use crate::world::World;

struct MessageMeta {
    name: DebugName,
    type_id: TypeId,
    update: fn(&mut World),
}

/// Registry of all message resources that should be updated together.
///
/// This type keeps a compact list of registered message types and the function
/// pointer needed to rotate each [`Messages<T>`] resource during
/// [`Self::run_updates`].
pub struct MessageRegistry {
    messages: Vec<MessageMeta>,
    indices: TypeIdMap<usize>,
}

impl Debug for MessageRegistry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list()
            .entries(self.messages.iter().map(|meta| meta.name))
            .finish()
    }
}

impl Resource for MessageRegistry {}

fn update_messages<T: Message>(world: &mut World) {
    if let Some(mut messages) = world.get_resource_mut::<Messages<T>>() {
        messages.update();
    }
}

impl MessageRegistry {
    pub(crate) const fn new() -> Self {
        Self {
            messages: Vec::new(),
            indices: TypeIdMap::new(),
        }
    }

    /// Registers a message type for global lifecycle updates.
    ///
    /// Registration is idempotent: registering the same type twice is a no-op.
    ///
    /// - Return `true` if it's new message.
    /// - Return `false` if it's already registered.
    pub fn register_message<T: Message>(&mut self) -> bool {
        let type_id = TypeId::of::<T>();
        if self.indices.contains(type_id) {
            return false;
        }

        let index = self.messages.len();
        self.messages.push(MessageMeta {
            type_id,
            name: DebugName::type_name::<T>(),
            update: update_messages::<T>,
        });

        self.indices.insert(type_id, index);

        true
    }

    /// Deregisters a message type from global lifecycle updates.
    ///
    /// - Return `true` if it's exist.
    /// - Return `false` if it's not exist.
    pub fn unregister_message<T: Message>(&mut self) -> bool {
        let type_id = TypeId::of::<T>();
        if let Some(index) = self.indices.remove(type_id) {
            self.messages.swap_remove(index);

            if let Some(type_id) = self.messages.get(index).map(|meta| meta.type_id) {
                self.indices.remove(type_id);
                self.indices.insert(type_id, index);
            }
            return true;
        }

        false
    }

    /// Updates all registered message resources.
    ///
    /// Call this once per update (for example after running schedules) so all
    /// message storages rotate in sync.
    pub fn run_updates(world: &mut World) {
        let unsafe_world = world.unsafe_world();
        let registry = unsafe { &unsafe_world.data_mut().message_registry };
        let world_mut = unsafe { unsafe_world.data_mut() };
        registry.messages.iter().for_each(|meta| (meta.update)(world_mut));
    }
}
