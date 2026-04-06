use core::fmt::Debug;
use core::marker::PhantomData;
use core::panic::{RefUnwindSafe, UnwindSafe};

use super::queue::RawCommandQueue;
use super::{Command, CommandQueue, EntityCommand};
use crate::bundle::Bundle;
use crate::entity::{Entity, FetchError};
use crate::error::{EcsError, ErrorHandler};
use crate::prelude::Resource;
use crate::system::{AccessTable, ReadOnlySystemParam, SystemMeta, SystemParam};
use crate::tick::Tick;
use crate::utils::DebugLocation;
use crate::world::{FromWorld, UnsafeWorld, World, WorldId};

// -----------------------------------------------------------------------------
// Commands

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

pub struct EntityCommands<'w, 's> {
    entity: Entity,
    commands: Commands<'w, 's>,
}

// -----------------------------------------------------------------------------
// Commands Implementation

impl Debug for Commands<'_, '_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Commands").field("world", &self.world.id()).finish()
    }
}

unsafe impl ReadOnlySystemParam for Commands<'_, '_> {}

unsafe impl SystemParam for Commands<'_, '_> {
    type State = CommandQueue;
    type Item<'world, 'state> = Commands<'world, 'state>;
    const DEFERRED: bool = true;
    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    #[track_caller]
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
    ) -> Result<Self::Item<'w, 's>, EcsError> {
        Ok(Commands::new(unsafe { world.read_only() }, state))
    }

    fn apply_deferred(state: &mut Self::State, _: &SystemMeta, world: &mut World) {
        state.apply(world);
    }
}

impl<'w, 's> Commands<'w, 's> {
    #[inline]
    pub fn new(world: &'w World, queue: &'s mut CommandQueue) -> Self {
        Commands {
            queue: queue.raw(),
            world,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub fn world_id(&self) -> WorldId {
        self.world.id()
    }

    #[inline]
    pub fn reborrow(&mut self) -> Commands<'_, '_> {
        Commands {
            queue: self.queue.clone(),
            world: self.world,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        unsafe { self.queue.is_empty() }
    }

    #[inline]
    pub fn with_entity(&mut self, entity: Entity) -> EntityCommands<'_, '_> {
        EntityCommands {
            entity,
            commands: self.reborrow(),
        }
    }

    #[inline]
    #[track_caller]
    pub fn try_with_entity(
        &mut self,
        entity: Entity,
    ) -> Result<EntityCommands<'_, '_>, FetchError> {
        let _ = self.world.entities.locate(entity)?;
        Ok(self.with_entity(entity))
    }

    #[inline]
    pub fn push(&mut self, cmd: impl Command) {
        unsafe {
            self.queue.push(cmd.handle_error());
        }
    }

    #[inline]
    pub fn push_handled(&mut self, cmd: impl Command, handler: ErrorHandler) {
        unsafe {
            self.queue.push(cmd.handle_error_with(handler));
        }
    }

    #[inline]
    pub fn queue_silenced(&mut self, cmd: impl Command) {
        unsafe {
            self.queue.push(cmd.ignore_error());
        }
    }

    #[inline]
    #[track_caller]
    pub fn spawn<B: Bundle>(&mut self, bundle: B) -> EntityCommands<'_, '_> {
        let caller = DebugLocation::caller();
        let entity = self.world.alloc_entity();

        unsafe {
            self.queue.push(move |world: &mut World| {
                world.spawn_at_with_caller(bundle, entity, caller);
            });
        }

        self.with_entity(entity)
    }

    #[inline]
    #[track_caller]
    pub fn spawn_batch<I>(&mut self, batch: I)
    where
        I: IntoIterator + Send + Sync + 'static,
        I::Item: Bundle,
    {
        let caller = DebugLocation::caller();
        unsafe {
            self.queue.push(move |world: &mut World| {
                world.spawn_batch_with_caller(batch, caller);
            });
        }
    }

    #[inline]
    pub fn init_resource<R: Resource + Send + FromWorld>(&mut self) {
        self.push(super::init_resource::<R>());
    }

    #[inline]
    pub fn insert_resource<R: Resource + Send>(&mut self, resource: R) {
        self.push(super::insert_resource::<R>(resource));
    }

    #[inline]
    pub fn remove_resource<R: Resource + Send>(&mut self) {
        self.push(super::remove_resource::<R>());
    }
}

// -----------------------------------------------------------------------------
// EntityCommands Implementation

impl<'w, 's> EntityCommands<'w, 's> {
    #[inline]
    pub fn reborrow(&mut self) -> EntityCommands<'_, '_> {
        EntityCommands {
            entity: self.entity,
            commands: self.commands.reborrow(),
        }
    }

    #[inline]
    pub fn push<C: EntityCommand<Output = ()>>(&mut self, cmd: C) {
        self.commands.push(cmd.with_entity(self.entity).handle_error());
    }
}
