use core::ops::Deref;

use crate::borrow::{NonSendMut, ResMut};
use crate::command::Commands;
use crate::entity::FetchError;
use crate::message::{Message, MessageId, MessageIdIterator};
use crate::prelude::Resource;
use crate::query::{Query, QueryData, QueryFilter};
use crate::world::{FetchEntities, UnsafeWorld, World};

/// A restricted mutable world handle for deferred mutation workflows.
///
/// `DeferredWorld` is designed for contexts where you need to:
/// - read world data immediately,
/// - enqueue structural changes through [`Commands`], and
/// - perform limited direct mutable access to entities/resources.
///
/// Conceptually, it wraps an [`UnsafeWorld`] and exposes an API surface that is
/// convenient for command/deferred execution paths.
///
/// Key properties:
/// - Dereferences to `&World` for read-only operations.
/// - Can still obtain selected mutable handles (for example, resource/entity
///   accessors and command queue writes).
/// - Works well in places where query initialization or full structural setup
///   should happen earlier on `&mut World`, while runtime code only consumes
///   pre-registered state.
pub struct DeferredWorld<'w>(UnsafeWorld<'w>);

impl<'w> Deref for DeferredWorld<'w> {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        unsafe { self.0.read_only() }
    }
}

impl<'w> From<&'w mut World> for DeferredWorld<'w> {
    fn from(value: &'w mut World) -> Self {
        DeferredWorld(value.unsafe_world())
    }
}

impl<'w> UnsafeWorld<'w> {
    /// Reinterprets this unsafe world view as a [`DeferredWorld`].
    ///
    /// # Safety
    ///
    /// Caller must uphold the aliasing and lifetime guarantees required by
    /// [`UnsafeWorld`].
    #[inline(always)]
    pub const unsafe fn deferred(self) -> DeferredWorld<'w> {
        DeferredWorld(self)
    }
}

impl World {
    /// Creates a [`DeferredWorld`] view from `&mut World`.
    ///
    /// This is the ergonomic entry point for deferred command-oriented flows.
    #[inline(always)]
    pub const fn deferred(&mut self) -> DeferredWorld<'_> {
        DeferredWorld(self.unsafe_world())
    }
}

impl<'w> DeferredWorld<'w> {
    /// Returns mutable access to the underlying [`World`] internals.
    ///
    /// This function cannot be public, it's actually unsafe.
    fn world_mut(&mut self) -> &mut World {
        unsafe { self.0.data_mut() }
    }

    /// Creates a shorter-lived reborrow of this deferred world handle.
    ///
    /// Useful when splitting borrows across helper calls.
    #[inline]
    pub fn reborrow(&mut self) -> DeferredWorld<'_> {
        Self(self.0)
    }

    /// Returns a [`Commands`] interface bound to this world and command queue.
    ///
    /// Commands are enqueued first and applied later by the normal flush path.
    #[inline]
    pub fn commands(&mut self) -> Commands<'_, '_> {
        let world = unsafe { self.0.read_only() };
        let queue = unsafe { &mut self.0.data_mut().command_queue };
        Commands::new(world, queue)
    }

    /// Returns mutable handles for one or more entities.
    #[inline]
    pub fn entity_mut<E: FetchEntities>(&mut self, entities: E) -> E::Mut<'_> {
        self.world_mut().entity_mut::<E>(entities)
    }

    /// Tries to return mutable handles for one or more entities.
    ///
    /// Returns [`FetchError`] when any requested entity cannot be fetched.
    #[inline]
    pub fn get_entity_mut<E: FetchEntities>(
        &mut self,
        entities: E,
    ) -> Result<E::Mut<'_>, FetchError> {
        self.world_mut().get_entity_mut::<E>(entities)
    }

    /// Returns mutable access to a `Send` resource if it exists.
    #[inline]
    pub fn get_resource_mut<R: Resource + Send>(&mut self) -> Option<ResMut<'_, R>> {
        self.world_mut().get_resource_mut::<R>()
    }

    /// Returns mutable access to a `Send` resource.
    ///
    /// Panics if the resource is not present.
    #[inline]
    pub fn resource_mut<R: Resource + Send>(&mut self) -> ResMut<'_, R> {
        self.world_mut().resource_mut::<R>()
    }

    /// Returns mutable access to a non-`Send` resource if it exists.
    #[inline]
    pub fn get_non_send_mut<R: Resource>(&mut self) -> Option<NonSendMut<'_, R>> {
        self.world_mut().get_non_send_mut::<R>()
    }

    /// Returns mutable access to a non-`Send` resource.
    ///
    /// Panics if the resource is not present.
    #[inline]
    pub fn non_send_mut<R: Resource>(&mut self) -> NonSendMut<'_, R> {
        self.world_mut().non_send_mut::<R>()
    }

    /// Writes one message into the registered message buffer.
    ///
    /// Returns `None` and logs an error when the message type is not
    /// registered. Register first via `World::register_message`.
    #[inline]
    pub fn write_message<M: Message>(&mut self, message: M) -> Option<MessageId<M>> {
        self.world_mut().write_message(message)
    }

    /// Writes a batch of messages into the registered message buffer.
    ///
    /// Returns `None` and logs an error when the message type is not
    /// registered. Register first via `World::register_message`.
    #[inline]
    pub fn write_message_batch<M: Message>(
        &mut self,
        messages: impl IntoIterator<Item = M>,
    ) -> Option<MessageIdIterator<M>> {
        self.world_mut().write_message_batch(messages)
    }

    /// Creates a query view from an already cached [`QueryState`].
    ///
    /// Returns `None` when the query state has not been registered.
    /// Register it first via [`World::register_query`] or call [`World::query`]
    /// / [`World::query_with`] to auto-register.
    #[inline]
    pub fn query_cached<D, F>(&mut self) -> Option<Query<'_, '_, D, F>>
    where
        D: QueryData + 'static,
        F: QueryFilter + 'static,
    {
        self.world_mut().query_cached()
    }
}
