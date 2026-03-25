use alloc::boxed::Box;
use core::panic::Location;

use crate::error::EcsError;
use crate::world::World;

/// A boxed deferred command with captured call-site information.
///
/// `CommandObject` stores a one-shot function that operates on [`World`],
/// along with the source location where the command was created. It is the
/// executable unit queued by deferred command buffers such as [`Commands`].
///
/// [`Commands`]: crate::command::Commands
pub struct CommandObject {
    location: &'static Location<'static>,
    function: Box<dyn FnOnce(&mut World) -> Result<(), EcsError> + Send + 'static>,
}

impl CommandObject {
    /// Creates a new command object from a closure.
    ///
    /// The caller location is recorded via [`track_caller`](core::panic::Location)
    /// so diagnostics can report where the command originated.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_ecs::prelude::*;
    /// use voker_ecs::command::CommandObject;
    ///
    /// let command = CommandObject::new(|world| {
    ///     let _ = world.entity_count();
    ///     Ok(())
    /// });
    /// # let _ = command;
    /// ```
    #[track_caller]
    #[inline(always)] // inline to avoid copying closures in the stack.
    pub fn new<F>(func: F) -> Self
    where
        F: Send + 'static,
        F: FnOnce(&mut World) -> Result<(), EcsError>,
    {
        Self {
            location: Location::caller(),
            function: Box::new(func),
        }
    }

    /// Returns the source location where this command was created.
    pub fn location(&self) -> Location<'static> {
        *self.location
    }

    /// Consumes and executes this command against the given world.
    ///
    /// Returns any execution error produced by the command closure.
    pub fn run(self, world: &mut World) -> Result<(), EcsError> {
        (self.function)(world)
    }
}
