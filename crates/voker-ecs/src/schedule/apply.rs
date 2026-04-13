use core::marker::PhantomData;

use crate::system::{AccessTable, System, SystemError};
use crate::system::{SystemFlags, SystemId, SystemInput};
use crate::tick::Tick;
use crate::world::{DeferredWorld, UnsafeWorld, World};

/// A schedule synchronization system used to apply deferred world mutations.
///
/// This type is intentionally a no-op when executed directly.
/// The scheduler/executor uses it as a barrier-like marker: before this system
/// runs, pending deferred buffers from prior deferred systems can be collected
/// and applied so later systems observe up-to-date world state.
///
/// `ApplyDeferred<S>` is generic so callers can create distinct marker types.
/// Different `S` values produce different system type identities.
#[repr(transparent)]
pub struct ApplyDeferred<S> {
    tick: Tick,
    _marker: PhantomData<S>,
}

unsafe impl<S> Send for ApplyDeferred<S> {}
unsafe impl<S> Sync for ApplyDeferred<S> {}

impl<S> Copy for ApplyDeferred<S> {}

impl<S> Clone for ApplyDeferred<S> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Creates an [`ApplyDeferred`] synchronization system.
///
/// This helper is typically inserted where deferred command visibility should
/// be enforced across an ordering boundary in a schedule.
///
/// The input parameter is only used at the type level. It allows choosing a
/// marker type `S` so each `ApplyDeferred<S>` can have its own system identity.
#[inline(always)]
pub fn apply_deferred<S: 'static>(_: S) -> ApplyDeferred<S> {
    ApplyDeferred {
        tick: Tick::new(0),
        _marker: PhantomData,
    }
}

impl<S: 'static> System for ApplyDeferred<S> {
    type Input = ();
    type Output = ();

    fn id(&self) -> SystemId {
        SystemId::of::<Self>()
    }

    fn flags(&self) -> SystemFlags {
        SystemFlags::EXCLUSIVE.union(SystemFlags::NON_SEND)
    }

    fn last_run(&self) -> Tick {
        self.tick
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.tick = last_run;
    }

    fn initialize(&mut self, _world: &mut World) -> AccessTable {
        AccessTable::new()
    }

    unsafe fn run(
        &mut self,
        _input: <Self::Input as SystemInput>::Data<'_>,
        _world: UnsafeWorld<'_>,
    ) -> Result<Self::Output, SystemError> {
        Ok(())
    }

    fn is_deferred(&self) -> bool {
        false
    }

    fn is_non_send(&self) -> bool {
        true
    }

    fn is_exclusive(&self) -> bool {
        true
    }

    fn defer(&mut self, _world: DeferredWorld) {}

    fn apply_deferred(&mut self, _world: &mut World) {}
}
