#![expect(clippy::module_inception, reason = "For better structure.")]

use crate::bundle::Bundle;
use crate::entity::{Entity, FetchError};
use crate::error::{ErrorContext, ErrorHandler, GameError, Severity, ToGameError};
use crate::link::LinkHookMode;
use crate::message::{Message, Messages};
use crate::prelude::ScheduleLabel;
use crate::resource::Resource;
use crate::system::{IntoSystem, SystemId, SystemInput};
use crate::utils::{DebugLocation, DebugName};
use crate::world::{EntityOwned, FromWorld, World};

// -----------------------------------------------------------------------------
// Command

/// A [`World`] mutation.
///
/// Should be used with [`Commands::push`](super::Commands::push).
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
///     commands.push(AddToCounter(123));
/// }
/// ```
pub trait Command: Send + Sized + 'static {
    /// The return type of [`apply`](Command::apply).
    type Output: ToGameError;

    /// Applies this command to the provided `world`.
    ///
    /// This method is used to define what a command "does" when it
    /// is ultimately applied.
    fn apply(self, world: &mut World) -> Self::Output;

    #[inline]
    fn with_severity(self, severity: Severity) -> impl Command<Output = Option<GameError>> {
        move |world: &mut World| self.apply(world).with_severity(severity)
    }

    #[inline]
    fn map_severity(
        self,
        f: impl FnOnce(Severity) -> Severity + Send + 'static,
    ) -> impl Command<Output = Option<GameError>> {
        move |world: &mut World| self.apply(world).map_severity(f)
    }

    /// Takes a [`Command`] that returns a Result and uses a given error handler
    /// function to convert it into a [`Command`].
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

    /// Takes a [`Command`] that returns a Result and uses the fallback error handler
    /// function to convert it into a [`Command`].
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

    /// Takes a [`Command`] that returns a Result and ignores any error that occurs.
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
/// Should be used with [`EntityCommands::push`](super::EntityCommands::push).
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
///     entity_cmd.push(insert_name);
/// }
/// ```
pub trait EntityCommand: Send + Sized + 'static {
    type Output: ToGameError;

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
    O: ToGameError,
{
    type Output = O;

    fn apply(self, world: &mut World) -> O {
        self(world)
    }
}

impl<O, F> EntityCommand for F
where
    F: FnOnce(EntityOwned) -> O + Send + 'static,
    O: ToGameError,
{
    type Output = O;

    fn apply(self, entity: EntityOwned) -> Self::Output {
        self(entity)
    }
}

// -----------------------------------------------------------------------------
// pre-defined Command

/// A [`Command`] that spawns a new entity from a [`Bundle`].
#[inline]
#[track_caller]
pub fn spawn<B: Bundle>(bundle: B) -> impl Command {
    let caller = DebugLocation::caller();
    move |world: &mut World| {
        world.spawn_with_caller(bundle, caller);
    }
}

/// A [`Command`] that spawns a new entity from a [`Bundle`].
#[inline]
#[track_caller]
pub fn spawn_at<B: Bundle>(bundle: B, entity: Entity) -> impl Command {
    let caller = DebugLocation::caller();
    move |world: &mut World| world.spawn_at_with_caller(bundle, entity, caller).map(|_| ())
}

/// A [`Command`] that consumes an iterator of [`Bundle`]s to spawn a series of entities.
///
/// This is more efficient than spawning the entities individually.
#[inline]
#[track_caller]
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

/// A [`Command`] that despawn a entity.
///
/// No-op if the entity does not exist
#[inline]
pub fn despawn(entity: Entity) -> impl Command {
    move |world: &mut World| {
        world.despawn(entity);
    }
}

/// A [`Command`] that despawn entities from iterator.
///
/// Simply ignore entities that won't spawn.
#[inline]
pub fn despawn_batch<I>(entity_iter: I) -> impl Command
where
    I: IntoIterator<Item = Entity> + Send + Sync + 'static,
{
    move |world: &mut World| {
        for entity in entity_iter {
            world.despawn(entity);
        }
    }
}

/// A [`Command`] that initialize [`Resource`] if it does not exist.
#[inline]
pub fn init_resource<R: Resource + Send + FromWorld>() -> impl Command {
    move |world: &mut World| {
        world.init_resource::<R>();
    }
}

/// A [`Command`] that initialize [`Resource`] if it does not exist.
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

/// Registers a system so it can later be called by [`Commands::run_system_cached`] or [`World::run_system_cached`].
///
/// [`Commands::run_system_cached`]: super::Commands::run_system_cached
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
    move |world: &mut World| -> Result<(), GameError> { world.run_system(system) }
}

/// A [`Command`] that runs the given system with the given input value,
/// caching its [`S`] in a [`CachedSystemId`](crate::system::CachedSystemId) resource.
#[inline]
pub fn run_system_with<I, M, S>(system: S, input: I::Data<'static>) -> impl Command
where
    I: SystemInput<Data<'static>: Send> + Send + 'static,
    M: 'static,
    S: IntoSystem<I, (), M> + Send + 'static,
{
    move |world: &mut World| -> Result<(), GameError> { world.run_system_with(system, input) }
}

/// A [`Command`] that runs the given system,
/// caching its [`SystemId`] in a [`CachedSystemId`](crate::system::CachedSystemId) resource.
#[inline]
#[track_caller]
pub fn run_system_cached<I>(id: SystemId, input: I::Data<'static>) -> impl Command
where
    I: SystemInput<Data<'static>: Send> + Send + 'static,
{
    let caller = DebugLocation::caller();
    move |world: &mut World| {
        world.run_system_cached::<I, ()>(id, input).map_err(|_| {
            GameError::warning(alloc::format!(
                "{caller}Run cached system failed, the given system\
                {id} is unregistered or input `{}` type mismatched",
                DebugName::type_name::<I>()
            ))
        })
    }
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
        world.resource_mut::<Messages<M>>().write(message);
    }
}

// -----------------------------------------------------------------------------
// pre-defined EntityCommand

/// A [`Command`] that insert a [`Bundle`] for a entity.
#[inline]
#[track_caller]
pub fn insert(bundle: impl Bundle) -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.insert_with_caller(bundle, LinkHookMode::Run, caller);
    }
}

/// A [`Command`] that insert a [`Bundle`] for a entity if it does not exist.
#[inline]
#[track_caller]
pub fn insert_if_new<T: Bundle>(bundle: impl FnOnce() -> T + Send + 'static) -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.insert_if_new_with_caller(bundle, LinkHookMode::Run, caller);
    }
}

/// A [`Command`] that remove a [`Bundle`]'s explict components for a entity.
#[inline]
#[track_caller]
pub fn remove<T: Bundle>() -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.remove_explicit_with_caller::<T>(caller);
    }
}

/// A [`Command`] that remove a [`Bundle`]'s explict components for a entity.
#[inline]
#[track_caller]
pub fn remove_explicit<T: Bundle>() -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.remove_explicit_with_caller::<T>(caller);
    }
}

/// A [`Command`] that remove a [`Bundle`]'s all components for a entity.
#[inline]
#[track_caller]
pub fn remove_required<T: Bundle>() -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.remove_required_with_caller::<T>(caller);
    }
}

/// A [`Command`] that clear all components for a entity.
#[inline]
#[track_caller]
pub fn clear() -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.clear_with_caller(caller);
    }
}
