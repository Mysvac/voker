use core::ops::Deref;

use crate::command::Commands;
use crate::world::{UnsafeWorld, World};

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
    #[inline(always)]
    pub const unsafe fn deferred(self) -> DeferredWorld<'w> {
        DeferredWorld(self)
    }
}

impl World {
    #[inline(always)]
    pub const fn deferred(&mut self) -> DeferredWorld<'_> {
        DeferredWorld(self.unsafe_world())
    }
}

impl<'w> DeferredWorld<'w> {
    pub fn reborrow(&mut self) -> DeferredWorld<'_> {
        Self(self.0)
    }

    pub fn commands(&mut self) -> Commands<'_, '_> {
        let world = unsafe { self.0.read_only() };
        let queue = unsafe { &mut self.0.data_mut().command_queue };
        Commands::new(world, queue)
    }
}
