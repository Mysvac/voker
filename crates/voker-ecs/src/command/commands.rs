use core::fmt::Debug;
use core::marker::PhantomData;
use core::panic::{RefUnwindSafe, UnwindSafe};

use super::queue::RawCommandQueue;
use super::{Command, CommandQueue, EntityCommand};
use crate::bundle::Bundle;
use crate::entity::{Entity, FetchError};
use crate::error::ErrorHandler;
use crate::event::Event;
use crate::message::Message;
use crate::observer::{IntoEntityObserver, IntoObserver};
use crate::prelude::{Resource, ScheduleLabel};
use crate::system::{AccessTable, SystemParam, SystemParamError};
use crate::system::{IntoSystem, SystemId, SystemInput, SystemMeta};
use crate::tick::Tick;
use crate::world::{DeferredWorld, FromWorld, UnsafeWorld, World, WorldId};

// -----------------------------------------------------------------------------
// Commands

/// A deferred world-mutation interface.
///
/// `Commands` collects operations into a command queue and applies them later,
/// typically at the end of a schedule stage.
///
/// This lets systems request structural world changes (spawn, insert/remove
/// components, resource updates) without requiring immediate exclusive access
/// to `World` during system execution.
///
/// Queued commands are applied later at deferred synchronization points.
///
/// # Examples
///
/// ```rust
/// use voker_ecs::prelude::*;
///
/// #[derive(Component, Clone)]
/// struct Tag;
///
/// fn setup(mut commands: Commands) {
///     commands.spawn(Tag);
/// }
/// ```
pub struct Commands<'w, 's> {
    queue: RawCommandQueue,
    world: &'w World,
    _marker: PhantomData<&'s CommandQueue>,
}

unsafe impl Sync for Commands<'_, '_> {}
unsafe impl Send for Commands<'_, '_> {}
impl UnwindSafe for Commands<'_, '_> {}
impl RefUnwindSafe for Commands<'_, '_> {}

// -----------------------------------------------------------------------------
// EntityCommands

/// Entity-scoped command builder.
///
/// `EntityCommands` wraps a target [`Entity`] plus a [`Commands`] handle, making
/// it ergonomic to enqueue multiple operations for the same entity.
///
/// # Examples
///
/// ```rust,ignore
/// use voker_ecs::prelude::*;
///
/// #[derive(Component, Clone)]
/// struct Hp(u32);
///
/// fn buff_player(mut commands: Commands, players: Query<Entity, Without<Hp>>) {
///     for entity in players {
///         commands.with_entity(entity).insert((Hp(150),));
///     }
/// }
/// ```
pub struct EntityCommands<'a> {
    entity: Entity,
    commands: Commands<'a, 'a>,
}

// -----------------------------------------------------------------------------
// SystemParam Implementation

impl Debug for Commands<'_, '_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Commands").field("world", &self.world.id()).finish()
    }
}

unsafe impl SystemParam for Commands<'_, '_> {
    type State = CommandQueue;
    type Item<'world, 'state> = Commands<'world, 'state>;
    const DEFERRED: bool = true;
    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn init_state(_world: &mut World) -> Self::State {
        CommandQueue::new()
    }

    fn mark_access(_table: &mut AccessTable, _state: &Self::State) -> bool {
        true
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        Ok(Commands::new(unsafe { world.read_only() }, state))
    }

    fn queue_deferred(state: &mut Self::State, _: &SystemMeta, mut world: DeferredWorld) {
        let mut commands = world.commands();
        unsafe {
            commands.queue.append(&mut state.raw());
        }
    }

    fn apply_deferred(state: &mut Self::State, _: &SystemMeta, world: &mut World) {
        state.apply(world);
    }
}

// -----------------------------------------------------------------------------
// Commands Implementation

impl<'w, 's> Commands<'w, 's> {
    /// Returns the id of the [`World`] bound to this command writer.
    #[inline]
    pub fn world_id(&self) -> WorldId {
        self.world.id()
    }

    /// Creates a command writer from a world view and a target queue.
    ///
    /// Most users obtain this through the [`SystemParam`] implementation.
    #[inline]
    pub fn new(world: &'w World, queue: &'s mut CommandQueue) -> Self {
        Commands {
            queue: queue.raw(),
            world,
            _marker: PhantomData,
        }
    }

    /// Returns a new `Commands` that writes to the provided
    /// [`CommandQueue`] instead of the one from `self`.
    ///
    /// Useful when composing APIs that stage commands into dedicated queues.
    #[inline]
    pub fn rebound_to<'q>(&self, queue: &'q mut CommandQueue) -> Commands<'w, 'q> {
        Commands {
            queue: queue.raw(),
            world: self.world,
            _marker: PhantomData,
        }
    }

    /// Appends all commands from `other` into this queue, leaving `other` empty.
    #[inline]
    pub fn append(&mut self, other: &mut CommandQueue) {
        unsafe {
            self.queue.append(&mut other.raw());
        }
    }

    /// Returns a reborrowed command writer with a shorter lifetime.
    #[inline]
    pub fn reborrow(&mut self) -> Commands<'_, '_> {
        Commands {
            queue: self.queue.clone(),
            world: self.world,
            _marker: PhantomData,
        }
    }

    /// Returns whether this queue currently has no pending commands.
    #[inline]
    pub fn is_empty(&self) -> bool {
        unsafe { self.queue.is_empty() }
    }

    /// Returns an [`EntityCommands`] handle for the given [`Entity`].
    ///
    /// Existence is validated when queued commands execute, not when queued.
    /// The entity may be despawned before application time.
    #[inline]
    pub fn with_entity(&mut self, entity: Entity) -> EntityCommands<'_> {
        EntityCommands {
            entity,
            commands: self.reborrow(),
        }
    }

    /// Returns an [`EntityCommands`] handle if the entity exists at call time.
    ///
    /// This is an eager validation helper only. The entity can still be
    /// despawned before queued commands are applied.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError`] if the requested entity does not currently exist.
    #[inline]
    pub fn try_with_entity(&mut self, entity: Entity) -> Result<EntityCommands<'_>, FetchError> {
        let _ = self.world.entities.locate(entity)?;
        Ok(self.with_entity(entity))
    }

    /// Pushes a generic [`Command`] to the queue.
    ///
    /// If the [`Command`] returns a [`Result`], it will be handled
    /// using the [fallback error handler](crate::error::FallbackErrorHandler).
    ///
    /// To use a custom error handler, see [`Commands::push_handled`].
    #[inline]
    pub fn push(&mut self, cmd: impl Command) {
        unsafe {
            self.queue.push(cmd.handle_error());
        }
    }

    /// Pushes a generic [`Command`] to the queue.
    ///
    /// If the [`Command`] returns a [`Result`],
    /// the given `error_handler` will be used to handle error cases.
    ///
    /// To implicitly use the fallback error handler, see [`Commands::push`].
    #[inline]
    pub fn push_handled(&mut self, cmd: impl Command, handler: ErrorHandler) {
        unsafe {
            self.queue.push(cmd.handle_error_with(handler));
        }
    }

    /// Pushes a generic [`Command`] and silently ignores command errors.
    #[inline]
    pub fn push_silenced(&mut self, cmd: impl Command) {
        unsafe {
            self.queue.push(cmd.ignore_error());
        }
    }

    /// Spawn a empty entity.
    ///
    /// This command is faster then `spawn(())`.`
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn spawn_empty(&mut self) -> EntityCommands<'_> {
        let entity = self.world.alloc_entity();

        self.push(super::spawn_empty_at(entity));

        self.with_entity(entity)
    }

    /// Enqueues a spawn operation and returns the corresponding [`EntityCommands`].
    ///
    /// To spawn many entities with the same combination of components,
    /// [`spawn_batch`](Self::spawn_batch) can be used for better performance.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn spawn<B: Bundle>(&mut self, bundle: B) -> EntityCommands<'_> {
        let entity = self.world.alloc_entity();

        self.push(super::spawn_at(bundle, entity));

        self.with_entity(entity)
    }

    /// Enqueues spawning multiple entities from a batch of [`Bundle`] values.
    ///
    /// A batch can be any type that implements [`IntoIterator`] and
    /// contains bundles, such as a [`Vec<Bundle>`](alloc::vec::Vec)
    /// or an array `[Bundle; N]`.
    ///
    /// This is equivalent to repeatedly calling [`spawn`](Self::spawn), but can
    /// be faster due to batched allocation and contiguous processing.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn spawn_batch<I>(&mut self, batch: I)
    where
        I: IntoIterator + Send + Sync + 'static,
        I::Item: Bundle,
    {
        self.push(super::spawn_batch(batch));
    }

    /// Despawns an entity and removes all of its components.
    ///
    /// Logs at info level if the entity is already despawned.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn despawn(&mut self, entity: Entity) {
        self.push(super::despawn(entity));
    }

    /// Despawns an entity and removes all of its components.
    ///
    /// No-op if the entity is already despawned.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn try_despawn(&mut self, entity: Entity) {
        self.push(super::try_despawn(entity));
    }

    /// Despawns many entities from iterator.
    ///
    /// No-op for entities that are already despawned.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn despawn_batch<I>(&mut self, batch: I)
    where
        I: IntoIterator<Item = Entity> + Send + Sync + 'static,
    {
        self.push(super::despawn_batch(batch));
    }

    /// Initializes a [`Resource`] in the [`World`] using [`FromWorld`].
    ///
    /// If the resource already exists, this is a no-op.
    ///
    /// The inferred value is determined by the [`FromWorld`] trait of the resource.
    /// Note that any resource with the [`Default`] trait automatically implements
    /// [`FromWorld`], and those default values will be used.
    #[inline]
    pub fn init_resource<R: Resource + Send + FromWorld>(&mut self) {
        self.push(super::init_resource::<R>());
    }

    /// Initializes a NonSend [`Resource`] in the [`World`] using [`FromWorld`].
    ///
    /// If the resource already exists, this is a no-op.
    #[inline]
    pub fn init_non_send<R: Resource + FromWorld>(&mut self) {
        self.push(super::init_non_send::<R>());
    }

    /// Inserts a [`Resource`] into the [`World`] with a specific value.
    ///
    /// This will overwrite any previous value of the same resource type.
    #[inline]
    pub fn insert_resource<R: Resource + Send>(&mut self, resource: R) {
        self.push(super::insert_resource::<R>(resource));
    }

    /// Removes a [`Resource`] from the [`World`] if it exists.
    #[inline]
    pub fn remove_resource<R: Resource + Send>(&mut self) {
        self.push(super::remove_resource::<R>());
    }

    /// Registers a system and returns its [`SystemId`] so it can later be called by
    /// [`Commands::run_system`] or [`World::run_system`].
    #[inline]
    pub fn register_system<I, O, M>(
        &mut self,
        system: impl IntoSystem<I, O, M> + Send + 'static,
    ) -> SystemId
    where
        I: SystemInput + Send + 'static,
        O: Send + 'static,
        M: 'static,
    {
        let system_id = system.system_id();
        self.push(super::register_system(system));
        system_id
    }

    /// Runs the system corresponding to the given [`IntoSystem`].
    #[inline]
    pub fn run_system<M, S>(&mut self, system: S)
    where
        M: 'static,
        S: IntoSystem<(), (), M> + Send + 'static,
    {
        self.push(super::run_system::<M, S>(system));
    }

    /// Runs the given system with the given input value,
    /// caching its [`SystemId`] in a resource.
    #[inline]
    pub fn run_system_with<I, M, S>(&mut self, system: S, input: I::Data<'static>)
    where
        I: SystemInput<Data<'static>: Send> + Send + 'static,
        M: 'static,
        S: IntoSystem<I, (), M> + Send + 'static,
    {
        self.push(super::run_system_with::<I, M, S>(system, input));
    }

    /// Runs the system corresponding to the given [`SystemId`] with input.
    ///
    /// Before running a cached system, it must first be registered via
    /// [`Commands::register_system`] or [`World::register_system`].
    #[inline]
    pub fn run_system_by_id<I>(&mut self, id: SystemId, input: I::Data<'static>)
    where
        I: SystemInput<Data<'static>: Send> + Send + 'static,
    {
        self.push(super::run_system_by_id::<I>(id, input));
    }

    /// Runs the schedule corresponding to the given [`ScheduleLabel`].
    #[inline]
    pub fn run_schedule(&mut self, label: impl ScheduleLabel) {
        self.push(super::run_schedule(label));
    }

    /// Writes an arbitrary [`Message`].
    #[inline]
    pub fn write_message<M: Message>(&mut self, message: M) {
        self.push(super::write_message(message));
    }

    /// Triggers the given [`Event`], which will run any [`Observer`]s watching for it.
    ///
    /// [`Observer`]: crate::observer::Observer
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn trigger<'a>(&mut self, event: impl Event<Trigger<'a>: Default>) {
        self.push(super::trigger(event));
    }

    /// Triggers the given [`Event`] using the given [`Trigger`], which will run any [`Observer`]s watching for it.
    ///
    /// [`Trigger`]: crate::event::Trigger
    /// [`Observer`]: crate::observer::Observer
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn trigger_with<E: Event<Trigger<'static>: Send + Sync>>(
        &mut self,
        event: E,
        trigger: E::Trigger<'static>,
    ) {
        self.push(super::trigger_with(event, trigger));
    }

    pub fn add_observer<M>(&mut self, observer: impl IntoObserver<M>) {
        self.push(super::add_observer(observer));
    }
}

// -----------------------------------------------------------------------------
// EntityCommands Implementation

impl<'a> EntityCommands<'a> {
    /// Returns the target [`Entity`] id.
    #[inline]
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// Returns an [`EntityCommands`] reborrow with a shorter lifetime.
    ///
    /// This is useful if you have `&mut EntityCommands` but you need `EntityCommands`.
    #[inline]
    pub fn reborrow(&mut self) -> EntityCommands<'_> {
        EntityCommands {
            entity: self.entity,
            commands: self.commands.reborrow(),
        }
    }

    /// Returns the underlying [`Commands`].
    #[inline]
    pub fn commands(&mut self) -> Commands<'_, '_> {
        self.commands.reborrow()
    }

    /// Returns a mutable reference to the underlying [`Commands`].
    #[inline]
    pub fn commands_mut(&mut self) -> &mut Commands<'a, 'a> {
        &mut self.commands
    }

    /// Pushes an [`EntityCommand`] for this entity.
    ///
    /// The [fallback error handler](crate::error::FallbackErrorHandler) will be used to handle error
    /// cases. Every [`EntityCommand`] checks whether the entity exists at the time of execution and
    /// returns an error if it does not.
    ///
    /// To use a custom error handler, see [`EntityCommands::push_handled`].
    #[inline]
    pub fn push(&mut self, command: impl EntityCommand) -> &mut Self {
        self.commands.push(command.with_entity(self.entity));
        self
    }

    /// Pushes an [`EntityCommand`] for this entity with a custom error handler.
    ///
    /// The given `error_handler` will be used to handle error cases. Every [`EntityCommand`] checks
    /// whether the entity exists at the time of execution and returns an error if it does not.
    ///
    /// To implicitly use the fallback error handler, see [`EntityCommands::push`].
    #[inline]
    pub fn push_handled(
        &mut self,
        command: impl EntityCommand,
        handler: ErrorHandler,
    ) -> &mut Self {
        self.commands.push_handled(command.with_entity(self.entity), handler);
        self
    }

    /// Pushes an [`EntityCommand`] for this entity and ignores errors.
    ///
    /// Unlike [`EntityCommands::push_handled`], this will completely ignore any errors that occur.
    #[inline]
    pub fn push_silenced(&mut self, command: impl EntityCommand) -> &mut Self {
        self.commands.push_silenced(command.with_entity(self.entity));
        self
    }

    /// Adds a [`Bundle`] of components to the entity.
    ///
    /// This will overwrite any previous value(s) of the same component type.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn insert(&mut self, bundle: impl Bundle) -> &mut Self {
        self.push(super::insert(bundle))
    }

    /// Adds a [`Bundle`] of components to the entity.
    ///
    /// Errors are ignored if the entity is despawned before command execution.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn try_insert(&mut self, bundle: impl Bundle) -> &mut Self {
        self.push_silenced(super::insert(bundle))
    }

    /// Removes all explicit component types in a [`Bundle`] from the entity.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn remove<B: Bundle>(&mut self) -> &mut Self {
        self.push(super::remove::<B>())
    }

    /// Removes all explicit component types in a [`Bundle`] from the entity.
    ///
    /// Errors are ignored if the entity is despawned before command execution.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn try_remove<B: Bundle>(&mut self) -> &mut Self {
        self.push_silenced(super::remove::<B>())
    }

    /// Removes all components from this entity.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn clear(&mut self) -> &mut Self {
        self.push(super::clear())
    }

    /// Removes all components from this entity.
    ///
    /// Errors are ignored if the entity is despawned before command execution.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn try_clear(&mut self) -> &mut Self {
        self.push_silenced(super::clear())
    }

    /// Despawns an entity and removes all of its components.
    ///
    /// Logs at info level if the entity is already despawned.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn despawn(mut self) {
        self.commands.despawn(self.entity);
    }

    /// Despawns an entity and removes all of its components.
    ///
    /// No-op if the entity is already despawned.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn try_despawn(mut self) {
        self.commands.try_despawn(self.entity);
    }

    #[inline]
    pub fn observe<M>(&mut self, observer: impl IntoEntityObserver<M>) -> &mut Self {
        self.push(super::observe(observer));
        self
    }

    #[inline]
    pub fn try_observe<M>(&mut self, observer: impl IntoEntityObserver<M>) -> &mut Self {
        self.push_silenced(super::observe(observer));
        self
    }

    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn clone(&mut self, linked_clone: bool) -> &mut Self {
        self.push(super::clone(linked_clone));
        self
    }

    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn try_clone(&mut self, linked_clone: bool) -> &mut Self {
        self.push_silenced(super::clone(linked_clone));
        self
    }
}
