#![expect(clippy::module_inception, reason = "For better structure.")]

use crate::bundle::Bundle;
use crate::entity::{Entity, FetchError};
use crate::error::{ErrorContext, ErrorHandler, GameError, IntoGameError, Severity};
use crate::event::Event;
use crate::message::{Message, MessageQueue};
use crate::observer::{IntoEntityObserver, IntoObserver};
use crate::prelude::ScheduleLabel;
use crate::resource::Resource;
use crate::system::{IntoSystem, SystemId, SystemInput};
use crate::utils::{DebugLocation, DebugName};
use crate::world::{EntityOwned, FromWorld, World};

// -----------------------------------------------------------------------------
// Command

/// A [`World`] mutation.
///
/// Should be used with [`Commands::queue`](super::Commands::queue).
///
/// The `Output` associated type is the returned "output" of the command.
///
/// # Examples
///
/// ```no_run
/// # use voker_ecs::prelude::*;
/// #
/// // Our world resource
/// #[derive(Resource, Default)]
/// struct Counter(u64);
///
/// // Our custom command
/// struct AddToCounter(u64);
///
/// impl Command for AddToCounter {
///     type Output = ();
///
///     fn apply(self, world: &mut World) {
///         let mut counter = world.resource_mut_or_init::<Counter>();
///         counter.0 += self.0;
///     }
/// }
///
/// fn some_system(mut commands: Commands) {
///     commands.queue(AddToCounter(123));
/// }
/// ```
pub trait Command: Send + Sized + 'static {
    /// The return type of [`apply`](Command::apply).
    type Output: IntoGameError;

    /// Applies this command to the provided `world`.
    ///
    /// This method is used to define what a command "does" when it
    /// is ultimately applied.
    fn apply(self, world: &mut World) -> Self::Output;

    /// Overrides the resulting error severity of this command.
    ///
    /// If this command succeeds, no error is produced.
    /// If it fails, the returned error severity is replaced with `severity`.
    #[inline]
    fn with_severity(self, severity: Severity) -> impl Command<Output = Option<GameError>> {
        move |world: &mut World| self.apply(world).with_severity(severity)
    }

    /// Merges `severity` into the resulting error severity of this command.
    ///
    /// This preserves the original severity information while escalating it
    /// with the provided value when needed.
    #[inline]
    fn merge_severity(self, severity: Severity) -> impl Command<Output = Option<GameError>> {
        move |world: &mut World| self.apply(world).merge_severity(severity)
    }

    /// Maps the resulting error severity of this command with a custom function.
    ///
    /// The mapper is only evaluated when this command produces an error.
    #[inline]
    fn map_severity(
        self,
        f: impl FnOnce(Severity) -> Severity + Send + 'static,
    ) -> impl Command<Output = Option<GameError>> {
        move |world: &mut World| self.apply(world).map_severity(f)
    }

    /// Converts a fallible [`Command`] into an infallible one using the
    /// provided error handler.
    ///
    /// The error handler internally handles an error if it occurs and returns `()`.
    #[inline]
    fn handle_error_with(self, handler: ErrorHandler) -> impl Command<Output = ()> {
        move |world: &mut World| {
            if let Some(e) = self.apply(world).to_err() {
                let name = DebugName::type_name::<Self>();
                handler(e, ErrorContext::Command { name });
            }
        }
    }

    /// Converts a fallible [`Command`] into an infallible one using the
    /// world's fallback error handler.
    ///
    /// The error handler internally handles an error if it occurs and returns `()`.
    #[inline]
    fn handle_error(self) -> impl Command<Output = ()> {
        move |world: &mut World| {
            if let Some(e) = self.apply(world).to_err() {
                let name = DebugName::type_name::<Self>();
                world.fallback_error_handler()(e, ErrorContext::Command { name });
            }
        }
    }

    /// Converts a fallible [`Command`] into an infallible one by ignoring errors.
    #[inline]
    fn ignore_error(self) -> impl Command<Output = ()> {
        move |world: &mut World| {
            let _ = self.apply(world);
        }
    }
}

// -----------------------------------------------------------------------------
// EntityCommand

/// A command which gets executed for a given [`Entity`].
///
/// Should be used with [`EntityCommands::queue`](super::EntityCommands::queue).
///
/// The `Output` associated type is the returned "output" of the command.
///
/// # Examples
///
/// ```no_run
/// # use voker_ecs::prelude::*;
/// #
/// fn insert_name(mut entity: EntityOwned) {
///     entity.insert(Name::from("foo"));
/// }
///
/// fn setup(mut commands: Commands) {
///     let mut entity_cmd = commands.spawn(());
///     entity_cmd.queue(insert_name);
/// }
/// ```
pub trait EntityCommand: Send + Sized + 'static {
    /// The return type of [`apply`](EntityCommand::apply).
    type Output: IntoGameError;

    /// Executes this command for the given [`Entity`].
    fn apply(self, entity: EntityOwned) -> Self::Output;

    /// Passes in a specific entity to an [`EntityCommand`], resulting in a
    /// [`Command`] that internally runs the [`EntityCommand`] on that entity.
    #[inline]
    fn with_entity(self, entity: Entity) -> impl Command {
        move |world: &mut World| -> Result<Self::Output, FetchError> {
            Ok(self.apply(world.get_entity_owned(entity)?))
        }
    }
}

// -----------------------------------------------------------------------------
// Implementation

impl<F, O> Command for F
where
    F: FnOnce(&mut World) -> O + Send + 'static,
    O: IntoGameError,
{
    type Output = O;

    fn apply(self, world: &mut World) -> O {
        self(world)
    }
}

impl<O, F> EntityCommand for F
where
    F: FnOnce(EntityOwned) -> O + Send + 'static,
    O: IntoGameError,
{
    type Output = O;

    fn apply(self, entity: EntityOwned) -> Self::Output {
        self(entity)
    }
}

// -----------------------------------------------------------------------------
// pre-defined Command

/// A [`Command`] that spawns a empty entity.
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn spawn_empty() -> impl Command {
    let caller = DebugLocation::caller();
    move |world: &mut World| {
        world.spawn_empty_with_caller(caller);
    }
}

/// A [`Command`] that spawns a empty entity.
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn spawn_empty_at(entity: Entity) -> impl Command {
    let caller = DebugLocation::caller();
    move |world: &mut World| world.spawn_empty_at_with_caller(entity, caller).err()
}

/// A [`Command`] that spawns a new entity from a [`Bundle`].
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn spawn<B: Bundle>(bundle: B) -> impl Command {
    let caller = DebugLocation::caller();
    move |world: &mut World| {
        world.spawn_with_caller(bundle, caller);
    }
}

/// A [`Command`] that spawns a new entity at a specific [`Entity`] id.
///
/// Returns an error if the target entity id cannot be used.
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn spawn_at<B: Bundle>(bundle: B, entity: Entity) -> impl Command {
    let caller = DebugLocation::caller();
    move |world: &mut World| world.spawn_at_with_caller(bundle, entity, caller).err()
}

/// A [`Command`] that consumes an iterator of [`Bundle`]s to spawn a series of entities.
///
/// This is more efficient than spawning the entities individually.
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn spawn_batch<I>(bundles_iter: I) -> impl Command
where
    I: IntoIterator + Send + Sync + 'static,
    I::Item: Bundle,
{
    let caller = DebugLocation::caller();
    move |world: &mut World| {
        world.spawn_batch_with_caller(bundles_iter, caller);
    }
}

/// A [`Command`] that despawns an entity.
///
/// Logs at info level if the entity does not exist.
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn despawn(entity: Entity) -> impl Command {
    let caller = DebugLocation::caller();
    move |world: &mut World| world.despawn_with_caller(entity, caller)
}

/// A [`Command`] that despawns an entity.
///
/// No-op if the entity does not exist.
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn try_despawn(entity: Entity) -> impl Command {
    let caller = DebugLocation::caller();
    move |world: &mut World| {
        world.try_despawn_with_caller(entity, caller);
    }
}

/// A [`Command`] that despawns entities from an iterator.
///
/// Ignores entities that do not exist.
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn despawn_batch<I>(entity_iter: I) -> impl Command
where
    I: IntoIterator<Item = Entity> + Send + Sync + 'static,
{
    let caller = DebugLocation::caller();
    move |world: &mut World| {
        for entity in entity_iter {
            world.try_despawn_with_caller(entity, caller);
        }
    }
}

/// A [`Command`] that initializes a [`Resource`] if it does not exist.
#[inline]
pub fn init_resource<R: Resource + Send + FromWorld>() -> impl Command {
    move |world: &mut World| {
        world.init_resource::<R>();
    }
}

/// A [`Command`] that initializes a non-send [`Resource`] if it does not exist.
#[inline]
pub fn init_non_send<R: Resource + FromWorld>() -> impl Command {
    move |world: &mut World| {
        world.init_non_send::<R>();
    }
}

/// A [`Command`] that inserts a [`Resource`] into the world.
#[inline]
pub fn insert_resource<R: Resource + Send>(resource: R) -> impl Command {
    move |world: &mut World| {
        world.insert_resource::<R>(resource);
    }
}

/// A [`Command`] that removes a [`Resource`] from the world.
#[inline]
pub fn remove_resource<R: Resource + Send>() -> impl Command {
    move |world: &mut World| {
        world.drop_resource::<R>();
    }
}

/// Registers a system so it can later be called by [`Commands::run_system_by_id`] or [`World::run_system_by_id`].
///
/// [`Commands::run_system_by_id`]: super::Commands::run_system_by_id
#[inline]
pub fn register_system<I, O, M>(system: impl IntoSystem<I, O, M> + Send + 'static) -> impl Command
where
    I: SystemInput + Send + 'static,
    O: Send + 'static,
    M: 'static,
{
    move |world: &mut World| {
        world.register_system(system);
    }
}

/// A [`Command`] that runs the system corresponding to the given [`IntoSystem`].
#[inline]
pub fn run_system<M, S>(system: S) -> impl Command
where
    M: 'static,
    S: IntoSystem<(), (), M> + Send + 'static,
{
    move |world: &mut World| world.run_system(system)
}

/// A [`Command`] that runs the given system with the given input value.
///
/// The system is registered and cached in the world's internal system cache,
/// then executed with the provided input.
#[inline]
pub fn run_system_with<I, M, S>(system: S, input: I::Data<'static>) -> impl Command
where
    I: SystemInput<Data<'static>: Send> + Send + 'static,
    M: 'static,
    S: IntoSystem<I, (), M> + Send + 'static,
{
    move |world: &mut World| world.run_system_with(system, input)
}

/// A [`Command`] that runs a cached system by id.
///
/// Returns an error if the system id is not registered.
#[inline]
pub fn run_system_by_id<I>(id: SystemId, input: I::Data<'static>) -> impl Command
where
    I: SystemInput<Data<'static>: Send> + Send + 'static,
{
    move |world: &mut World| world.run_system_by_id::<I, ()>(id, input)
}

/// A [`Command`] that runs the schedule corresponding to the given [`ScheduleLabel`].
#[inline]
pub fn run_schedule(label: impl ScheduleLabel) -> impl Command {
    move |world: &mut World| {
        world.run_schedule(label);
    }
}

/// A [`Command`] that writes an arbitrary [`Message`].
#[inline]
pub fn write_message<M: Message>(message: M) -> impl Command {
    move |world: &mut World| {
        world.resource_mut::<MessageQueue<M>>().write(message);
    }
}

/// Triggers the given [`Event`], which will run any [`Observer`]s watching for it.
///
/// [`Observer`]: crate::observer::Observer
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn trigger<'a, E: Event<Trigger<'a>: Default>>(mut event: E) -> impl Command {
    let caller = DebugLocation::caller();
    move |world: &mut World| {
        let mut trigger = <E::Trigger<'_> as Default>::default();
        world.trigger_with_caller(&mut event, &mut trigger, caller);
    }
}

/// Triggers the given [`Event`] using the given [`Trigger`], which will run any [`Observer`]s watching for it.
///
/// [`Trigger`]: crate::event::Trigger
/// [`Observer`]: crate::observer::Observer
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn trigger_with<E: Event<Trigger<'static>: Send + Sync>>(
    mut event: E,
    mut trigger: E::Trigger<'static>,
) -> impl Command {
    let caller = DebugLocation::caller();
    move |world: &mut World| {
        world.trigger_with_caller(&mut event, &mut trigger, caller);
    }
}

/// Adds a global [`Observer`] to the [`World`].
///
/// The observer will run when matching events are triggered.
///
/// [`Observer`]: crate::observer::Observer
#[inline]
pub fn add_observer<M>(observer: impl IntoObserver<M>) -> impl Command {
    move |world: &mut World| {
        world.add_observer(observer);
    }
}

// -----------------------------------------------------------------------------
// pre-defined EntityCommand

/// An [`EntityCommand`] that inserts a [`Bundle`] into an entity.
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn insert(bundle: impl Bundle) -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.insert_with_caller(bundle, caller);
    }
}

/// An [`EntityCommand`] that inserts a [`Bundle`] into an entity if missing.
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn insert_if_new<T: Bundle>(bundle: impl FnOnce() -> T + Send + 'static) -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.insert_if_new_with_caller(bundle, caller);
    }
}

/// An [`EntityCommand`] that removes a bundle's explicit components from an entity.
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn remove<T: Bundle>() -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.remove_explicit_with_caller::<T>(caller);
    }
}

/// An [`EntityCommand`] that removes a bundle's explicit components from an entity.
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn remove_explicit<T: Bundle>() -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.remove_explicit_with_caller::<T>(caller);
    }
}

/// An [`EntityCommand`] that removes a bundle's required components from an entity.
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn remove_required<T: Bundle>() -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.remove_required_with_caller::<T>(caller);
    }
}

/// An [`EntityCommand`] that clears all components from an entity.
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn clear() -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.clear_with_caller(caller);
    }
}

/// An [`EntityCommand`] that clones an entity.
///
/// If `linked_clone` is `true`, the clone keeps an entity-link relationship
/// with the source entity when supported by clone behavior.
#[inline]
#[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
pub fn clone(linked_clone: bool) -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.clone_with_caller(linked_clone, caller);
    }
}

/// Adds an entity-scoped [`Observer`] to this entity.
///
/// The observer is invoked only for events matching this specific entity.
///
/// [`Observer`]: crate::observer::Observer
#[inline]
pub fn observe<M>(observer: impl IntoEntityObserver<M>) -> impl EntityCommand {
    move |mut entity: EntityOwned| {
        entity.observe(observer);
    }
}
