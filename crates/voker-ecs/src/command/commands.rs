use core::fmt::Debug;

use alloc::vec::Vec;

use super::CommandObject;
use crate::bundle::Bundle;
use crate::command::EntityCommands;
use crate::entity::Entity;
use crate::error::EcsError;
use crate::system::{AccessTable, ReadOnlySystemParam, SystemParam};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World, WorldId};

/// A deferred command buffer used to optimize System parallelism.
///
/// Functions submitted via [`Commands`] are not executed immediately, but are
/// instead submitted to the World's deferred command queue. Since the command
/// queue is thread-safe, `Commands` is considered to not access any components
/// or resources, thereby optimizing System parallelism.
///
/// For performance optimization, `Commands` maintains a local command buffer.
/// Commands are first accumulated in this local buffer and only transferred
/// to the global command queue when [`flush`] is called. The local buffer is
/// automatically flushed when the `Commands` instance is dropped.
///
/// For a single `Commands` instance, commands are guaranteed to execute in
/// order. However, when multiple `Commands` instances exist concurrently,
/// commands from different instances are interleaved in the global queue
/// based on when each instance flushes its local buffer. This can affect the
/// relative order of commands from different sources. To control ordering,
/// users can explicitly call [`flush`] to submit accumulated commands to the
/// global queue at specific points.
///
/// # Execution Timing
///
/// Flushing only enqueues commands into `World`'s global command queue.
/// Actual execution happens later when `World::apply_commands()` runs
/// (typically at the end of `Schedule::run`).
///
/// [`flush`]: Commands::flush
///
/// # Examples
///
/// ```no_run
/// use voker_ecs::prelude::*;
///
/// #[derive(Component)]
/// struct Disabled;
///
/// fn despawn_entities(
///     mut commands: Commands,
///     query: Query<Entity, With<Disabled>>,
/// ) {
///     for entity in query {
///         commands.despawn(entity);
///     }
/// }
/// ```
pub struct Commands<'a> {
    world: &'a World,
    buffer: Vec<CommandObject>,
}

unsafe impl ReadOnlySystemParam for Commands<'_> {}

unsafe impl SystemParam for Commands<'_> {
    type State = ();
    type Item<'world, 'state> = Commands<'world>;
    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(_world: &mut World) -> Self::State {}

    fn mark_access(_table: &mut AccessTable, _state: &Self::State) -> bool {
        true
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        _state: &'s mut Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, EcsError> {
        Ok(Commands {
            world: unsafe { world.read_only() },
            buffer: Vec::new(),
        })
    }
}

impl Debug for Commands<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Commands")
            .field("world", &self.world_id())
            .finish()
    }
}

impl Drop for Commands<'_> {
    fn drop(&mut self) {
        self.flush();
    }
}

impl<'a> Commands<'a> {
    /// Flushes all commands from the local buffer to the global queue.
    ///
    /// The submitted commands maintain their original order.
    ///
    /// This does not execute commands immediately; it only transfers them to
    /// the world's deferred queue.
    ///
    /// Note that this function will be called in [`Drop::drop`] automatically.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_ecs::prelude::*;
    ///
    /// # #[derive(Component)]
    /// # struct Foo;
    /// #
    /// fn example(mut commands: Commands) {
    ///     commands.spawn(Foo);
    ///     commands.flush(); // optional
    /// }
    /// ```
    pub fn flush(&mut self) {
        if !self.buffer.is_empty() {
            let commands = ::core::mem::take(&mut self.buffer);
            self.world.command_queue.extend(commands);
        }
    }

    /// Creates a new `Commands` instance associated with the given world.
    #[inline]
    #[must_use]
    pub fn new(world: &'a World) -> Self {
        Self {
            world,
            buffer: Vec::new(),
        }
    }

    /// Returns the ID of the world associated with this command buffer.
    #[inline]
    #[must_use]
    pub fn world_id(&self) -> WorldId {
        self.world.id()
    }

    /// Creates a new `Commands` instance that shares the same world.
    ///
    /// This method flushes any pending commands in the current buffer before
    /// returning a new instance. The new instance starts with an empty buffer,
    /// ensuring that commands from the original instance are submitted in
    /// the correct order relative to commands from the new instance.
    ///
    /// This is mainly used to pass a short-lived command view into helper
    /// functions while preserving enqueue order.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_ecs::prelude::*;
    ///
    /// # #[derive(Component)]
    /// # struct Foo;
    /// #
    /// fn example(mut commands: Commands) {
    ///     fn helper(mut commands: Commands) {
    ///         /* ...... */
    ///     }
    ///     
    ///     helper(commands.reborrow());
    ///     commands.spawn(Foo);
    /// }
    /// ```
    #[must_use]
    pub fn reborrow(&mut self) -> Commands<'_> {
        self.flush();
        Commands {
            world: self.world,
            buffer: Vec::new(),
        }
    }

    /// Allocates a new entity ID without spawning it.
    ///
    /// This entity is uninitialized, can be used for [`Commands::spawn_in`].
    #[must_use]
    pub fn alloc_entity(&self) -> Entity {
        self.world.alloc_entity()
    }

    /// Pushes a custom command function into the buffer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_ecs::prelude::*;
    ///
    /// # #[derive(Component)]
    /// # struct Foo;
    /// #
    /// fn example(mut commands: Commands) {
    ///     commands.push(|world| {
    ///         if world.entity_count() == 0 {
    ///             world.spawn(Foo);
    ///         }
    ///         Ok(())
    ///     });
    /// }
    /// ```
    #[inline]
    #[track_caller]
    pub fn push<F>(&mut self, func: F)
    where
        F: Send + 'static,
        F: FnOnce(&mut World) -> Result<(), EcsError>,
    {
        self.buffer.push(CommandObject::new(func));
    }

    /// Spawns an entity with the given bundle at a specific entity ID.
    ///
    /// Flushes any pending commands in the current buffer and return an
    /// `EntityCommands` instance for further operations on the spawned entity.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_ecs::prelude::*;
    ///
    /// # #[derive(Component)]
    /// # struct Foo;
    /// #
    /// fn example(mut commands: Commands) {
    ///     let entity = commands.alloc_entity();
    ///     commands.spawn_in(Foo, entity);
    /// }
    /// ```
    #[inline]
    #[track_caller]
    pub fn spawn_in<B: Bundle>(&mut self, bundle: B, entity: Entity) -> EntityCommands<'_> {
        self.buffer.push(CommandObject::new(move |world| {
            world.entities.can_spawn(entity)?;
            world.spawn_in(bundle, entity);
            Ok(())
        }));

        self.with_entity(entity)
    }

    /// Spawns an entity with the given bundle.
    ///
    /// Flushes any pending commands in the current buffer and return an
    /// `EntityCommands` instance for further operations on the spawned entity.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_ecs::prelude::*;
    ///
    /// # #[derive(Component)]
    /// # struct Foo;
    /// #
    /// fn example(mut commands: Commands) {
    ///     let entity_cmd = commands.spawn(Foo);
    ///     entity_cmd.despawn();
    /// }
    /// ```
    #[inline]
    #[track_caller]
    pub fn spawn<B: Bundle>(&mut self, bundle: B) -> EntityCommands<'_> {
        let entity = self.world.alloc_entity();

        self.buffer.push(CommandObject::new(move |world| {
            world.spawn_in(bundle, entity);
            Ok(())
        }));

        self.with_entity(entity)
    }

    /// Despawns an entity.
    ///
    /// The entity and all its components will be removed.
    /// Any subsequent operations on this entity will fail.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_ecs::prelude::*;
    ///
    /// # #[derive(Component)]
    /// # struct Foo;
    /// #
    /// fn example(mut commands: Commands) {
    ///     let entity = commands.spawn(Foo).entity();
    ///     commands.despawn(entity);
    /// }
    /// ```
    #[inline]
    #[track_caller]
    pub fn despawn(&mut self, entity: Entity) {
        self.buffer.push(CommandObject::new(move |world| {
            world.despawn(entity).map_err(Into::into)
        }));
    }

    /// Attempts to despawn an entity, silently ignoring failures.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_ecs::prelude::*;
    ///
    /// # #[derive(Component)]
    /// # struct Foo;
    /// #
    /// fn example(mut commands: Commands) {
    ///     let entity = commands.alloc_entity();
    ///     commands.try_despawn(entity);
    ///     // ↑ This is safe, but this entity will leak
    ///     // because it will not be recycled
    /// }
    /// ```
    #[inline]
    #[track_caller]
    pub fn try_despawn(&mut self, entity: Entity) {
        self.buffer.push(CommandObject::new(move |world| {
            let _ = world.despawn(entity);
            Ok(())
        }));
    }

    /// Return an `EntityCommands` instance for further operations on the spawned entity.
    ///
    /// This function flushes any pending commands in the current buffer,
    /// to ensure the orderliness of the commands.
    #[inline]
    #[must_use]
    pub fn with_entity(&mut self, entity: Entity) -> EntityCommands<'_> {
        self.flush();

        EntityCommands {
            entity,
            commands: Commands::new(self.world),
        }
    }
}
