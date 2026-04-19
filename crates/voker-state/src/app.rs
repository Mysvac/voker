use voker_app::{App, MainScheduleOrder, Plugin, PreStartup, PreUpdate, SubApp};
use voker_ecs::message::MessageQueue;
use voker_ecs::schedule::{IntoSystemConfig, SystemSet};
use voker_ecs::world::FromWorld;

use crate::scoped::despawn_entities_on_enter_state;
use crate::scoped::despawn_entities_on_exit_state;
use crate::scoped::despawn_entities_when_state;
use crate::state::{DerivedStates, ManualStates, NextState, State, States, SubStates};
use crate::state::{StateTransition, StateTransitionSignal, StateTransitionSystems};

// -----------------------------------------------------------------------------
// StatesPlugin

#[derive(Default)]
/// Registers state-transition processing into the main schedule order.
///
/// This plugin ensures the [`StateTransition`] schedule runs before regular
/// update logic and startup logic so queued transitions are applied early.
pub struct StatesPlugin;

impl Plugin for StatesPlugin {
    fn build(&self, app: &mut App) {
        let world = app.world_mut();

        let mut order = world.resource_mut::<MainScheduleOrder>();
        order.insert_after(PreUpdate, StateTransition);
        order.insert_startup_before(PreStartup, StateTransition);

        let schedule = world.schedule_entry(StateTransition);

        schedule.add_system_set(StateTransitionSystems::Apply);
        schedule.add_system_set(StateTransitionSystems::Exit);
        schedule.add_system_set(StateTransitionSystems::Transition);
        schedule.add_system_set(StateTransitionSystems::Enter);

        // Set ordering follows voker's begin/end boundary style.
        schedule
            .config(
                StateTransitionSystems::Exit
                    .begin()
                    .after_set(StateTransitionSystems::Apply),
            )
            .config(
                StateTransitionSystems::Transition
                    .begin()
                    .after_set(StateTransitionSystems::Exit),
            )
            .config(
                StateTransitionSystems::Enter
                    .begin()
                    .after_set(StateTransitionSystems::Transition),
            );
    }
}

// -----------------------------------------------------------------------------
// AppStatesExt for SubApp

/// State installation methods for [`App`] and [`SubApp`].
///
/// These helpers initialize resources and transition systems for state-driven
/// workflows.
pub trait AppStatesExt {
    /// Initializes [`State<S>`] and [`NextState<S>`] with `S::from_world`.
    ///
    /// This operation is idempotent for each state type.
    ///
    /// Requires [`StatesPlugin`] to be installed for transitions to run.
    fn init_state<S: ManualStates + FromWorld>(&mut self) -> &mut Self;

    /// Inserts an explicit initial [`State<S>`].
    ///
    /// If the state already exists, this overwrites the current value and
    /// refreshes the initial transition signal.
    fn insert_state<S: ManualStates>(&mut self, state: S) -> &mut Self;

    /// Registers a derived-state type and its transition systems.
    ///
    /// This operation is idempotent for each state type.
    fn add_derived_state<S: DerivedStates>(&mut self) -> &mut Self;

    /// Registers a sub-state type and its transition systems.
    ///
    /// This operation is idempotent for each state type.
    fn add_sub_state<S: SubStates>(&mut self) -> &mut Self;
}

impl AppStatesExt for SubApp {
    fn init_state<S: ManualStates + FromWorld>(&mut self) -> &mut Self {
        warn_if_no_states_plugin_installed(self);

        let world = self.world_mut();

        if !world.contains_resource::<State<S>>() {
            world.init_resource::<State<S>>();
            world.init_resource::<NextState<S>>();
            world.register_message::<StateTransitionSignal<S>>();

            let schedule = world
                .schedules
                .get_mut(StateTransition)
                .unwrap_or_else(|| missing_plugins());

            S::register_state(schedule);

            let state = world.resource::<State<S>>().get().clone();

            world.write_message(StateTransitionSignal {
                exited: None,
                entered: Some(state),
                // makes no difference: the state didn't exist before anyways
                allow_same_state_transitions: true,
            });

            enable_state_scoped_entities::<S>(self);
        } else {
            let name = core::any::type_name::<S>();
            log::warn!("State {name} is already initialized.");
        }

        self
    }

    fn insert_state<S: ManualStates>(&mut self, state: S) -> &mut Self {
        warn_if_no_states_plugin_installed(self);

        let world = self.world_mut();

        if !world.contains_resource::<State<S>>() {
            world.insert_resource::<State<S>>(State::new(state.clone()));
            world.init_resource::<NextState<S>>();
            world.register_message::<StateTransitionSignal<S>>();

            let schedule = world
                .schedules
                .get_mut(StateTransition)
                .unwrap_or_else(|| missing_plugins());

            S::register_state(schedule);

            world.write_message(StateTransitionSignal {
                exited: None,
                entered: Some(state),
                // makes no difference: the state didn't exist before anyways
                allow_same_state_transitions: true,
            });

            enable_state_scoped_entities::<S>(self);
        } else {
            // Overwrite previous state and initial event
            world.insert_resource::<State<S>>(State::new(state.clone()));
            world.resource_mut::<MessageQueue<StateTransitionSignal<S>>>().clear();
            world.write_message(StateTransitionSignal {
                exited: None,
                entered: Some(state),
                // Not configurable for the moment. This controls whether inserting a state
                // with the same value as a pre-existing state should run state transitions.
                // Leaving it at `true` makes state insertion idempotent. Neat!
                allow_same_state_transitions: true,
            });
        }

        self
    }

    fn add_derived_state<S: DerivedStates>(&mut self) -> &mut Self {
        warn_if_no_states_plugin_installed(self);

        let world = self.world_mut();

        if !world.contains_resource::<MessageQueue<StateTransitionSignal<S>>>() {
            world.register_message::<StateTransitionSignal<S>>();

            let schedule = world
                .schedules
                .get_mut(StateTransition)
                .unwrap_or_else(|| missing_plugins());

            S::register_derived_state(schedule);

            let state = world.get_resource::<State<S>>().map(|s| s.get().clone());

            self.world_mut().write_message(StateTransitionSignal {
                exited: None,
                entered: state,
                allow_same_state_transitions: S::ALLOW_SAME_STATE_TRANSITIONS,
            });

            enable_state_scoped_entities::<S>(self);
        } else {
            let name = core::any::type_name::<S>();
            log::warn!("Derived state {name} is already initialized.");
        }

        self
    }

    fn add_sub_state<S: SubStates>(&mut self) -> &mut Self {
        warn_if_no_states_plugin_installed(self);

        let world = self.world_mut();

        if !world.contains_resource::<MessageQueue<StateTransitionSignal<S>>>() {
            world.init_resource::<NextState<S>>();
            world.register_message::<StateTransitionSignal<S>>();

            let schedule = world
                .schedules
                .get_mut(StateTransition)
                .unwrap_or_else(|| missing_plugins());

            S::register_sub_state(schedule);

            let state = world.get_resource::<State<S>>().map(|s| s.get().clone());

            world.write_message(StateTransitionSignal {
                exited: None,
                entered: state,
                // makes no difference: the state didn't exist before anyways
                allow_same_state_transitions: true,
            });

            enable_state_scoped_entities::<S>(self);
        } else {
            let name = core::any::type_name::<S>();
            log::warn!("Sub state {name} is already initialized.");
        }

        self
    }
}

#[cold]
#[inline(never)]
fn missing_plugins() -> ! {
    panic!(
        "The `StateTransition` schedule is missing. Did you forget \
        to add StatesPlugin or DefaultPlugins before calling init_state?"
    )
}

fn warn_if_no_states_plugin_installed(app: &SubApp) {
    if !app.is_plugin_added::<StatesPlugin>() {
        voker_os::once_expr!(log::warn!(
            "States were added to the app, but `StatesPlugin` is not installed."
        ));
    }
}

/// Enables scoped-entity cleanup systems for state `S`.
///
/// This wires despawn systems into transition phases so entity cleanup can be
/// driven by transition signals and marker components in [`crate::scoped`].
fn enable_state_scoped_entities<S: States>(app: &mut SubApp) {
    if !app
        .world()
        .contains_resource::<MessageQueue<StateTransitionSignal<S>>>()
    {
        let name = core::any::type_name::<S>();
        log::warn!(
            "State scoped entities are enabled for state `{name}`, but the state wasn't initialized in the app!"
        );
    }

    app.edit_schedule(StateTransition, |schedule| {
        schedule
            .add_systems(despawn_entities_on_exit_state::<S>.in_set(StateTransitionSystems::Exit))
            .add_systems(despawn_entities_on_enter_state::<S>.in_set(StateTransitionSystems::Enter))
            .add_systems(
                despawn_entities_when_state::<S>.in_set(StateTransitionSystems::Transition),
            );
    });
}

// -----------------------------------------------------------------------------
// AppStatesExt for App

impl AppStatesExt for App {
    fn init_state<S: ManualStates + FromWorld>(&mut self) -> &mut Self {
        self.main_mut().init_state::<S>();
        self
    }

    fn insert_state<S: ManualStates>(&mut self, state: S) -> &mut Self {
        self.main_mut().insert_state::<S>(state);
        self
    }

    fn add_derived_state<S: DerivedStates>(&mut self) -> &mut Self {
        self.main_mut().add_derived_state::<S>();
        self
    }

    fn add_sub_state<S: SubStates>(&mut self) -> &mut Self {
        self.main_mut().add_sub_state::<S>();
        self
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use core::fmt::Debug;
    use core::hash::Hash;

    use voker_app::App;
    use voker_ecs::message::MessageQueue;
    use voker_ecs::world::FromWorld;

    use crate::app::{AppStatesExt, StatesPlugin};
    use crate::state::{DerivedStates, ManualStates, NextState, State, SubStates};
    use crate::state::{StateTransitionSignal, States};

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    enum TestState {
        Menu,
        InGame,
    }

    impl FromWorld for TestState {
        fn from_world(_: &mut voker_ecs::world::World) -> Self {
            Self::Menu
        }
    }

    impl States for TestState {}
    impl ManualStates for TestState {}

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    enum TestDerived {
        Active,
    }

    impl DerivedStates for TestDerived {
        type SourceStates = (Option<TestState>,);

        fn derive(sources: Self::SourceStates) -> Option<Self> {
            match sources {
                (Some(TestState::InGame),) => Some(Self::Active),
                _ => None,
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    enum TestSubState {
        Hud,
    }

    impl States for TestSubState {}

    impl ManualStates for TestSubState {}

    impl SubStates for TestSubState {
        type SourceStates = (Option<TestState>,);

        fn should_exist(sources: Self::SourceStates) -> Option<Self> {
            match sources {
                (Some(TestState::InGame),) => Some(Self::Hud),
                _ => None,
            }
        }
    }

    #[test]
    fn init_state_inserts_resources_and_initial_value() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);

        app.init_state::<TestState>();

        assert!(app.world().contains_resource::<State<TestState>>());
        assert!(app.world().contains_resource::<NextState<TestState>>());
        assert!(
            app.world()
                .contains_resource::<MessageQueue<StateTransitionSignal<TestState>>>()
        );
        assert_eq!(
            app.world().resource::<State<TestState>>().get(),
            &TestState::Menu
        );
    }

    #[test]
    fn insert_state_overwrites_existing_state_value() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);

        app.insert_state(TestState::Menu);
        app.insert_state(TestState::InGame);

        assert_eq!(
            app.world().resource::<State<TestState>>().get(),
            &TestState::InGame
        );
    }

    #[test]
    fn add_derived_state_registers_transition_message_queue() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);

        app.init_state::<TestState>();
        app.add_derived_state::<TestDerived>();

        assert!(
            app.world()
                .contains_resource::<MessageQueue<StateTransitionSignal<TestDerived>>>()
        );
    }

    #[test]
    fn add_sub_state_registers_next_state_and_message_queue() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);

        app.init_state::<TestState>();
        app.add_sub_state::<TestSubState>();

        assert!(app.world().contains_resource::<NextState<TestSubState>>());
        assert!(
            app.world()
                .contains_resource::<MessageQueue<StateTransitionSignal<TestSubState>>>()
        );
    }
}
