//! Deferred-mutation synchronization barrier for schedules.
//!
//! [`ApplyDeferred`] is a no-op marker system inserted into a schedule graph
//! to signal a point where pending deferred buffers must be flushed before
//! subsequent systems execute. The free function [`apply_deferred`] provides
//! the most common single-type variant.

use core::marker::PhantomData;

use crate::system::{AccessTable, InternedSystemSet, System, SystemError};
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
pub struct ApplyDeferred<S> {
    id: SystemId,
    last_run: Tick,
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
pub fn apply_deferred<S: 'static>() -> ApplyDeferred<S> {
    ApplyDeferred {
        id: SystemId::of::<ApplyDeferred<S>>(),
        last_run: Tick::new(0),
        _marker: PhantomData,
    }
}

#[inline(always)]
pub fn apply_deferred_of_val<S: 'static>(_: S) -> ApplyDeferred<S> {
    ApplyDeferred {
        id: SystemId::of::<ApplyDeferred<S>>(),
        last_run: Tick::new(0),
        _marker: PhantomData,
    }
}

impl<S: 'static> System for ApplyDeferred<S> {
    type Input = ();
    type Output = ();

    fn id(&self) -> SystemId {
        self.id
    }

    fn flags(&self) -> SystemFlags {
        SystemFlags::NO_OP
            .union(SystemFlags::NON_SEND)
            .union(SystemFlags::EXCLUSIVE)
    }

    fn last_run(&self) -> Tick {
        self.last_run
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.last_run = last_run;
    }

    fn check_ticks(&mut self, now: Tick) {
        self.last_run.check_tick(now);
    }

    fn initialize(&mut self, _world: &mut World) -> AccessTable {
        AccessTable::new()
    }

    fn system_set(&self) -> InternedSystemSet {
        self.id.system_set()
    }

    fn set_system_set(&mut self, set: InternedSystemSet) {
        self.id = self.id.with_system_set(set);
    }

    unsafe fn run_raw(
        &mut self,
        _input: <Self::Input as SystemInput>::Data<'_>,
        _world: UnsafeWorld<'_>,
    ) -> Result<Self::Output, SystemError> {
        Ok(())
    }

    fn is_no_op(&self) -> bool {
        // NO_OP，Then we can optimize duplicated
        // `apply_deferred<T>` systems in executor.
        true
    }

    fn is_deferred(&self) -> bool {
        // As an exclusive target, the executor will
        // apply_defered for systems before this system run.
        // but this system does not need apply deferred.
        false
    }

    fn is_non_send(&self) -> bool {
        true
    }

    fn is_exclusive(&self) -> bool {
        true
    }

    fn queue_deferred(&mut self, _world: DeferredWorld) {}

    fn apply_deferred(&mut self, _world: &mut World) {}
}
