use core::fmt::Debug;

use super::Commands;
use crate::bundle::Bundle;
use crate::entity::Entity;
use crate::error::EcsError;
use crate::world::{EntityOwned, WorldId};

/// A command proxy that records deferred operations for a specific entity.
///
/// `EntityCommands` is returned by APIs such as [`Commands::spawn`] and
/// [`Commands::with_entity`], and provides ergonomic methods to queue
/// operations targeting one entity.
///
/// Like [`Commands`], submitted operations are deferred and are not executed
/// immediately. They are first accumulated in an internal local buffer and
/// transferred to the world's global command queue when [`flush`] is called.
/// The local buffer is also flushed automatically when this value is dropped.
///
/// For a single `EntityCommands` instance, queued operations execute in order.
/// Relative ordering with operations from other command buffers depends on
/// when each buffer is flushed.
///
/// [`flush`]: EntityCommands::flush
pub struct EntityCommands<'a> {
    pub(super) entity: Entity,
    pub(super) commands: Commands<'a>,
}

impl Debug for EntityCommands<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EntityCommand")
            .field("world", &self.commands.world_id())
            .field("entity", &self.entity)
            .finish()
    }
}

impl<'a> EntityCommands<'a> {
    /// Flushes all queued operations for this entity into the global queue.
    ///
    /// The submitted operations maintain their original order.
    ///
    /// Note that this is also called automatically when this value is dropped.
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
    ///     let mut entity = commands.spawn(Foo);
    ///     entity.flush(); // optional
    /// }
    /// ```
    pub fn flush(&mut self) {
        self.commands.flush();
    }

    /// Returns the world ID associated with this command buffer.
    #[must_use]
    pub fn world_id(&self) -> WorldId {
        self.commands.world_id()
    }

    /// Returns the target entity of this command proxy.
    #[must_use]
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// Creates a new `EntityCommands` that targets the same entity.
    ///
    /// This method flushes pending operations before creating the new value,
    /// ensuring operation ordering is preserved across borrows.
    #[must_use]
    pub fn reborrow(&mut self) -> EntityCommands<'_> {
        EntityCommands {
            entity: self.entity,
            commands: self.commands.reborrow(),
        }
    }

    /// Pushes a custom deferred operation for this entity.
    ///
    /// The command is executed later with an [`EntityOwned`] handle if the
    /// target entity still exists when the command queue is applied.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_ecs::prelude::*;
    ///
    /// # #[derive(Component)]
    /// # struct Disabled;
    /// #
    /// fn example(mut commands: Commands) {
    ///     let mut entity = commands.spawn(());
    ///     entity.push(|mut entity| {
    ///         entity.insert(Disabled);
    ///         Ok(())
    ///     });
    /// }
    /// ```
    #[inline]
    #[track_caller]
    pub fn push<F>(&mut self, func: F)
    where
        F: Send + 'static,
        F: FnOnce(EntityOwned) -> Result<(), EcsError>,
    {
        let entity = self.entity;
        self.commands.push(move |world| {
            let location = world.entities.locate(entity)?;
            func(EntityOwned {
                world: world.into(),
                entity,
                location,
            })
        });
    }

    /// Despawns the target entity.
    ///
    /// The entity and all its components will be removed.
    /// Any subsequently queued operations for this entity may fail.
    #[inline]
    #[track_caller]
    pub fn despawn(mut self) {
        self.commands.despawn(self.entity);
    }

    /// Attempts to despawn the target entity, silently ignoring failures.
    #[inline]
    #[track_caller]
    pub fn try_despawn(mut self) {
        self.commands.try_despawn(self.entity);
    }

    /// Inserts a bundle into the target entity.
    ///
    /// This operation is deferred and will run when commands are applied.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_ecs::prelude::*;
    ///
    /// # #[derive(Component)]
    /// # struct Health(u32);
    /// #
    /// fn example(mut commands: Commands) {
    ///     let mut entity = commands.spawn(());
    ///     entity.insert(Health(100));
    /// }
    /// ```
    #[inline]
    #[track_caller]
    pub fn insert<B: Bundle>(&mut self, bundle: B) {
        self.push(move |mut entity| {
            entity.insert(bundle);
            Ok(())
        });
    }

    /// Removes a bundle from the target entity.
    ///
    /// This operation is deferred and will run when commands are applied.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_ecs::prelude::*;
    ///
    /// # #[derive(Component)]
    /// # struct Health(u32);
    /// #
    /// fn example(mut commands: Commands) {
    ///     let mut entity = commands.spawn(Health(100));
    ///     entity.remove::<Health>();
    /// }
    /// ```
    #[inline]
    #[track_caller]
    pub fn remove<B: Bundle>(&mut self) {
        self.push(move |mut entity| {
            entity.remove::<B>();
            Ok(())
        });
    }
}
