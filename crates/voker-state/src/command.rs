use voker_ecs::command::Commands;
use voker_ecs::world::World;

use crate::state::{ManualStates, NextState};

// -----------------------------------------------------------------------------
// StateCommandExt

/// Extension helpers for [`Commands`] to enqueue state transitions.
pub trait StateCommandExt {
    /// Sets the next state the app should move to.
    ///
    /// Internally this schedules a command that updates the
    /// [`NextState<S>`](crate::prelude::NextState) resource with `state`.
    ///
    /// Note that commands introduce sync points to the ECS schedule, so modifying `NextState`
    /// directly may be more efficient depending on your use-case.
    fn set_state<S: ManualStates>(&mut self, state: S);

    /// Sets the next state the app should move to, skipping any state transitions
    /// if the next state is the same as the current state.
    ///
    /// Internally this schedules a command that updates the
    /// [`NextState<S>`](crate::prelude::NextState) resource with `state`.
    ///
    /// Note that commands introduce sync points to the ECS schedule, so modifying `NextState`
    /// directly may be more efficient depending on your use-case.
    fn set_state_if_neq<S: ManualStates>(&mut self, state: S);
}

impl StateCommandExt for Commands<'_, '_> {
    fn set_state<S: ManualStates>(&mut self, state: S) {
        self.queue(move |w: &mut World| {
            let mut next = w.resource_mut::<NextState<S>>();
            if let NextState::PendingIfNeq(prev) = &*next {
                tracing::debug!("overwriting next state {prev:?} with {state:?}");
            }
            next.set(state);
        });
    }

    fn set_state_if_neq<S: ManualStates>(&mut self, state: S) {
        self.queue(move |w: &mut World| {
            let mut next = w.resource_mut::<NextState<S>>();
            if let NextState::PendingIfNeq(prev) = &*next
                && *prev != state
            {
                tracing::debug!("overwriting next state {prev:?} with {state:?} if not equal");
            }
            next.set_if_neq(state);
        });
    }
}
