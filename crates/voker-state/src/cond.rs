use voker_ecs::borrow::{Res, ResRef};
use voker_ecs::tick::DetectChanges;

use crate::state::{State, States};

/// A condition system that passes only when `State<S>` exists.
pub fn state_exists<S: States>(current_state: Option<Res<State<S>>>) -> bool {
    current_state.is_some()
}

/// Returns a run condition that passes only when `State<S>` equals `expected`.
pub fn in_state<S: States>(expected: S) -> impl Fn(Option<Res<State<S>>>) -> bool + Clone {
    move |state: Option<Res<State<S>>>| match state {
        Some(state) => state.0 == expected,
        None => false,
    }
}

/// Returns a run condition that passes when `State<S>` changed in this tick.
pub fn state_changed<S: States>() -> impl Fn(Option<ResRef<State<S>>>) -> bool + Clone {
    |state: Option<ResRef<State<S>>>| state.map(|state| state.is_changed()).unwrap_or(false)
}
