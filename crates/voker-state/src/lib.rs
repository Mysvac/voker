#![doc = "Finite state machine"]
#![cfg_attr(docsrs, expect(internal_features, reason = "needed for fake_variadic"))]
#![cfg_attr(docsrs, feature(doc_cfg, rustdoc_internals))]
#![forbid(unsafe_code)]
#![no_std]

extern crate self as voker_state;

crate::cfg::std! { extern crate std; }
extern crate alloc;

pub mod cfg {
    voker_cfg::define_alias! {
        #[cfg(feature = "std")] => std,
    }
}

/// Command helpers for queuing state transitions.
pub mod command;
/// Run conditions for state-aware system execution.
pub mod cond;
/// Components and systems for state-scoped entity cleanup.
pub mod scoped;
/// Core state traits, resources, schedules, and transition systems.
pub mod state;

mod app;
pub use app::*;

pub use voker_state_derive as derive;

/// Common imports for state-driven gameplay code.
pub mod prelude {
    pub use crate::app::{AppStatesExt, StatesPlugin};
    pub use crate::command::CommandExt;
    pub use crate::cond::{in_state, state_changed, state_exists};
    pub use crate::scoped::{DespawnOnEnter, DespawnOnExit, DespawnWhen};
    pub use crate::state::{DerivedStates, ManualStates, NextState, State};
    pub use crate::state::{OnEnter, OnExit, OnTransition};
    pub use crate::state::{PreviousState, StateSet, States, SubStates};
    pub use crate::state::{StateTransition, StateTransitionSignal, StateTransitionSystems};
}
