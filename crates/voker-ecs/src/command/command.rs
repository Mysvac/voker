#![expect(clippy::module_inception, reason = "For better structure.")]

use crate::bundle::Bundle;
use crate::entity::{Entity, FetchError};
use crate::error::{EcsError, ErrorContext, ErrorHandler};
use crate::resource::Resource;
use crate::utils::{DebugLocation, DebugName};
use crate::world::{EntityOwned, FromWorld, World};

// -----------------------------------------------------------------------------
// CommandOutput

/// A trait implemented for types that can be used as the output of a [`Command`].
///
/// By default, the following types can be used as `CommandOutput`:
///
/// - `()`: Represents success with no error.
/// - `Option<T> where T: CommandOutput`: Returns `None` for success, `Some` recursively
///   processes the inner error.
/// - `Result<T, E> where T: CommandOutput, E: Into<EcsError>`: Returns `Err(e)` for failure,
///   `Ok(v)` recursively processes the inner output.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid `Command` output type",
    label = "invalid `Command` output type",
    note = "the output type should be `()`, or a `Option/Result` that can be converted into `EcsError`"
)]
pub trait CommandOutput: Sized {
    fn to_err(this: Self) -> Option<EcsError>;
}

impl CommandOutput for () {
    #[inline(always)]
    fn to_err(_: Self) -> Option<EcsError> {
        None
    }
}

impl<T: CommandOutput> CommandOutput for Option<T> {
    fn to_err(this: Self) -> Option<EcsError> {
        this.and_then(CommandOutput::to_err)
    }
}

impl<T: CommandOutput, E: Into<EcsError>> CommandOutput for Result<T, E> {
    fn to_err(this: Self) -> Option<EcsError> {
        match this {
            Ok(v) => CommandOutput::to_err(v),
            Err(e) => Some(e.into()),
        }
    }
}

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
///         let mut counter = world.resource_mut_or_init();
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
    type Output: CommandOutput;

    /// Applies this command to the provided `world`.
    ///
    /// This method is used to define what a command "does" when it
    /// is ultimately applied.
    fn apply(self, world: &mut World) -> Self::Output;

    /// Takes a [`Command`] that returns a Result and uses a given error handler
    /// function to convert it into a [`Command`].
    ///
    /// The error handler internally handles an error if it occurs and returns `()`.
    #[inline]
    fn handle_error_with(self, handler: ErrorHandler) -> impl Command<Output = ()> {
        move |world: &mut World| {
            if let Some(e) = CommandOutput::to_err(self.apply(world)) {
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
            if let Some(e) = CommandOutput::to_err(self.apply(world)) {
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
///     entity_cmd..push(insert_name);
/// }
/// ```
pub trait EntityCommand: Send + Sized + 'static {
    type Output: CommandOutput;

    /// Executes this command for the given [`Entity`].
    fn apply(self, entity: EntityOwned) -> Self::Output;

    /// Passes in a specific entity to an [`EntityCommand`], resulting in a [`Command`] that
    /// internally runs the [`EntityCommand`] on that entity.
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
    O: CommandOutput,
{
    type Output = O;

    fn apply(self, world: &mut World) -> O {
        self(world)
    }
}

impl<O, F> EntityCommand for F
where
    F: FnOnce(EntityOwned) -> O + Send + 'static,
    O: CommandOutput,
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

/// A [`Command`] that initialize [`Resource`] if it does not exist.
#[inline]
pub fn init_resource<R: Resource + Send + FromWorld>() -> impl Command {
    move |world: &mut World| {
        world.init_resource::<R>();
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

// -----------------------------------------------------------------------------
// pre-defined EntityCommand

/// A [`Command`] that insert a [`Bundle`] for a entity.
#[inline]
#[track_caller]
pub fn insert(bundle: impl Bundle) -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.insert_with_caller(bundle, caller);
    }
}

/// A [`Command`] that remove a [`Bundle`] for a entity.
#[inline]
#[track_caller]
pub fn remove<T: Bundle>() -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |mut entity: EntityOwned| {
        entity.remove_with_caller::<T>(caller);
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

/// A [`Command`] that despawn a entity.
#[inline]
#[track_caller]
pub fn despawn() -> impl EntityCommand {
    let caller = DebugLocation::caller();
    move |entity: EntityOwned| {
        entity.despawn_with_caller(caller);
    }
}
