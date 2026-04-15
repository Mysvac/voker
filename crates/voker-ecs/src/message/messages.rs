use alloc::vec::Vec;
use core::any::TypeId;
use core::fmt::Debug;

use voker_utils::extra::TypeIdMap;

use super::{Message, MessageId, MessageQueue};
use crate::prelude::ResourceId;
use crate::resource::Resources;
use crate::utils::DebugName;
use crate::world::World;

// -----------------------------------------------------------------------------
// MessageMeta

/// Compact runtime metadata for a registered message type.
///
/// This struct stores the stable `MessageId`, the debug-friendly type name,
/// the `TypeId` used for lookups, the `ResourceId` of the backing
/// `MessageQueue<T>` resource, and a function pointer used to update (rotate)
/// the queue during `Messages::run_updates`.
pub struct MessageMeta {
    id: MessageId,
    name: DebugName,
    type_id: TypeId,
    resource_id: ResourceId,
    update: fn(&mut World),
}

impl Debug for MessageMeta {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Message")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

impl MessageMeta {
    /// Returns the message's unique ID.
    #[inline(always)]
    pub fn id(&self) -> MessageId {
        self.id
    }

    /// Returns the message's debug name.
    #[inline(always)]
    pub fn name(&self) -> DebugName {
        self.name
    }

    /// Returns the message's [`TypeId`].
    #[inline(always)]
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns the message's [`TypeId`].
    #[inline(always)]
    pub fn resource_id(&self) -> ResourceId {
        self.resource_id
    }
}

fn update_queue<T: Message>(world: &mut World) {
    if let Some(mut queue) = world.get_resource_mut::<MessageQueue<T>>() {
        queue.update();
    }
}

// -----------------------------------------------------------------------------
// MessageMeta

/// Registry of all registered message types and their metadata.
pub struct Messages {
    metas: Vec<MessageMeta>,
    mapper: TypeIdMap<MessageId>,
}

impl Debug for Messages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.metas, f)
    }
}

impl Messages {
    /// Creates a new empty message registry.
    #[inline]
    pub(crate) const fn new() -> Self {
        Self {
            metas: Vec::new(),
            mapper: TypeIdMap::new(),
        }
    }

    /// Rotate all registered message queues once.
    ///
    /// This iterates the stored metadata and calls each message-type's stored
    /// update function. The update function rotates the underlying
    /// `MessageQueue<T>` so writers' messages become readable in the next
    /// update. This is usually invoked by `World::update_messages` once per
    /// frame/update.
    pub(crate) fn run_updates(world: &mut World) {
        let unsafe_world = world.unsafe_world();
        let messages = unsafe { &unsafe_world.data_mut().messages };
        let world_mut = unsafe { unsafe_world.data_mut() };
        messages.metas.iter().for_each(|meta| (meta.update)(world_mut));
    }
    /// Register message type `T` and ensure its backing resource exists.
    ///
    /// If `T` is already registered this returns the existing `MessageId`.
    /// Otherwise it allocates a new `MessageId`, registers a `MessageQueue<T>`
    /// resource in `ress`, and stores a `MessageMeta` entry for later updates.
    pub(crate) fn register<T: Message>(&mut self, ress: &mut Resources) -> MessageId {
        if let Some(id) = self.mapper.get(TypeId::of::<T>()) {
            return *id;
        }
        let id = MessageId::new(self.metas.len() as u32);
        let meta = MessageMeta {
            id,
            name: DebugName::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            resource_id: ress.register::<MessageQueue<T>>(),
            update: update_queue::<T>,
        };
        self.metas.push(meta);
        self.mapper.insert(TypeId::of::<T>(), id);
        id
    }
}

impl Messages {
    /// Returns the number of registered message types.
    #[inline]
    #[expect(clippy::len_without_is_empty, reason = "useless")]
    pub const fn len(&self) -> usize {
        self.metas.len()
    }

    /// Looks up a message ID by its [`TypeId`].
    #[inline]
    pub fn get_id(&self, type_id: TypeId) -> Option<MessageId> {
        self.mapper.get(type_id).copied()
    }

    /// Returns the message info for the given ID.
    #[inline]
    pub fn get(&self, id: MessageId) -> Option<&MessageMeta> {
        self.metas.get(id.index())
    }

    /// Returns the message info for the given ID without bounds checking.
    ///
    /// # Safety
    /// The caller must ensure `id` is a valid ID (i.e., `id.index() < self.len()`).
    #[inline]
    pub unsafe fn get_unchecked(&self, id: MessageId) -> &MessageMeta {
        debug_assert!(id.index() < self.metas.len());
        unsafe { self.metas.get_unchecked(id.index()) }
    }

    /// Extracts a slice containing the entire message metadata registry.
    #[inline]
    pub fn as_slice(&self) -> &[MessageMeta] {
        self.metas.as_slice()
    }

    /// Returns an iterator over the `MessageMeta` values.
    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, MessageMeta> {
        self.metas.iter()
    }
}
