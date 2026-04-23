#![doc = "Finite state machine"]
#![cfg_attr(docsrs, expect(internal_features, reason = "needed for fake_variadic"))]
#![cfg_attr(docsrs, feature(doc_cfg, rustdoc_internals))]
#![forbid(unsafe_code)]
#![no_std]

extern crate self as voker_state;

extern crate alloc;

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

#[cfg(test)]
mod tests {
    use voker_app::{App, PreStartup};
    use voker_ecs::prelude::{Commands, ResMut, Resource};

    use crate::app::{AppStatesExt, StatesPlugin};
    use crate::derive::{States, SubStates};
    use crate::state::{OnEnter, State};

    #[test]
    fn state_transition_before_pre_startup() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);

        #[derive(States, Default, PartialEq, Eq, Hash, Debug, Clone)]
        enum TestState {
            #[default]
            A,
        }

        #[derive(SubStates, Default, PartialEq, Eq, Hash, Debug, Clone)]
        #[source(TestState = TestState::A)]
        struct TestSubState;

        #[derive(Resource, Default, PartialEq, Eq, Debug)]
        struct Thingy(usize);

        app.init_state::<TestState>();
        app.add_sub_state::<TestSubState>();

        app.add_systems(OnEnter(TestState::A), |mut commands: Commands| {
            commands.init_resource::<Thingy>();
        });

        app.add_systems(PreStartup, |mut thingy: ResMut<Thingy>| {
            thingy.0 += 1;
        });

        assert!(!app.world().contains_resource::<State<TestSubState>>());

        app.update();

        // This assert only succeeds if first OnEnter(TestState::A) runs, followed by PreStartup.
        assert_eq!(app.world().resource::<Thingy>(), &Thingy(1));
        assert!(app.world().contains_resource::<State<TestSubState>>());
    }
}
