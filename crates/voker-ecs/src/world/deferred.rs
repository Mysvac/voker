use core::ops::Deref;

use crate::borrow::{NonSendMut, ResMut};
use crate::command::Commands;
use crate::component::HookContext;
use crate::entity::{Entity, FetchError};
use crate::message::{Message, MessageId, MessageIdIter};
use crate::prelude::{Component, ComponentId, Resource};
use crate::query::{Query, QueryData, QueryFilter};
use crate::system::{SystemError, SystemId, SystemInput};
use crate::utils::DebugLocation;
use crate::world::{EntityFetcher, FetchEntities, GetComponents, UnsafeWorld, World};

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

    /// Returns change-aware mutable component access for a single entity.
    ///
    /// Returns `None` when the entity is not spawned or the requested
    /// component pattern is unavailable.
    #[inline]
    pub fn get_mut<C: GetComponents>(&mut self, entity: Entity) -> Option<C::Mut<'_>> {
        self.world_mut().get_mut::<C>(entity)
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

    /// Returns mutable handles for one or more entities.
    #[inline]
    pub fn entity_mut<E: FetchEntities>(&mut self, entities: E) -> E::Mut<'_> {
        self.world_mut().entity_mut::<E>(entities)
    }

    /// Simultaneously provides access to entity data and a command queue, which
    /// will be applied when the [`World`] is next flushed.
    ///
    /// This allows using borrowed entity data to construct commands where the
    /// borrow checker would otherwise prevent it.
    ///
    /// See [`World::entities_and_commands`] for the non-deferred version.
    #[inline]
    pub fn entities_and_commands(&mut self) -> (EntityFetcher<'_>, Commands<'_, '_>) {
        unsafe { self.0.data_mut().entities_and_commands() }
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
    ) -> Option<MessageIdIter<M>> {
        self.world_mut().write_message_batch(messages)
    }

    /// Creates a query with default filter from deferred world context.
    ///
    /// Returns `None` when query state cannot be initialized or accessed.
    #[inline]
    pub fn try_query<D: QueryData + 'static>(&mut self) -> Option<Query<'_, '_, D>> {
        self.world_mut().try_query::<D>()
    }

    /// Creates a query with explicit filter from deferred world context.
    ///
    /// Returns `None` when query state cannot be initialized or accessed.
    #[inline]
    pub fn try_query_with<D, F>(&mut self) -> Option<Query<'_, '_, D, F>>
    where
        D: QueryData + 'static,
        F: QueryFilter + 'static,
    {
        self.world_mut().try_query_with::<D, F>()
    }

    /// Runs a registered system by id and returns the input back on cache miss.
    #[inline]
    pub fn run_system_by_id<'a, I, O>(
        &mut self,
        id: SystemId,
        input: I::Data<'a>,
    ) -> Result<O, SystemError>
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        self.world_mut().run_system_by_id::<I, O>(id, input)
    }

    /// Mutates component `T` on `entity` while preserving hook semantics.
    ///
    /// This mirrors [`World::modify_component`] for deferred execution paths.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn modify_component<T: Component, R>(
        &mut self,
        entity: Entity,
        f: impl FnOnce(&mut T) -> R,
    ) -> Result<Option<R>, FetchError> {
        let caller = DebugLocation::caller();
        self.world_mut().modify_component_with_caller(entity, caller, f)
    }

    #[inline]
    pub(crate) fn modify_component_with_caller<T: Component, R>(
        &mut self,
        entity: Entity,
        caller: DebugLocation,
        f: impl FnOnce(&mut T) -> R,
    ) -> Result<Option<R>, FetchError> {
        self.world_mut().modify_component_with_caller(entity, caller, f)
    }
}

macro_rules! define_trigger {
    ($func:ident, $hook:ident) => {
        /// # Safety
        /// Caller ensures that these components exist
        #[inline]
        pub(crate) unsafe fn $func(
            &mut self,
            entity: Entity,
            targets: impl Iterator<Item = ComponentId>,
            caller: DebugLocation,
        ) {
            for id in targets {
                // SAFETY: Caller ensures that these components exist
                let info = unsafe { self.components.get_unchecked(id) };
                if let Some(hook) = info.$hook() {
                    hook(DeferredWorld(self.0), HookContext { entity, id, caller });
                }
            }
        }
    };
}

#[expect(unused, reason = "todo")]
impl<'w> DeferredWorld<'w> {
    define_trigger!(trigger_on_add, on_add);
    define_trigger!(trigger_on_clone, on_clone);
    define_trigger!(trigger_on_insert, on_insert);
    define_trigger!(trigger_on_remove, on_remove);
    define_trigger!(trigger_on_discard, on_discard);
    define_trigger!(trigger_on_despawn, on_despawn);
}
