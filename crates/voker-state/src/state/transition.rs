use core::marker::PhantomData;
use core::mem;

use voker_ecs::borrow::ResMut;
use voker_ecs::command::Commands;
use voker_ecs::message::{Message, MessageReader, MessageWriter};
use voker_ecs::schedule::{ScheduleLabel, SystemSet};
use voker_ecs::system::In;
use voker_ecs::world::World;

use crate::state::{PreviousState, State, States};

// -----------------------------------------------------------------------------
// OnEnter

/// Runs when `State<S>` enters the given state variant.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct OnEnter<S: States>(pub S);

// -----------------------------------------------------------------------------
// OnExit

/// Runs when `State<S>` exits the given state variant.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct OnExit<S: States>(pub S);

// -----------------------------------------------------------------------------
// OnTransition

/// Runs when `State<S>` exits and enters a specific pair.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct OnTransition<S: States> {
    /// Previously active state value.
    pub exited: S,
    /// Newly entered state value.
    pub entered: S,
}

// -----------------------------------------------------------------------------
// StateTransition

/// Schedule that applies queued state transitions.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct StateTransition;

// -----------------------------------------------------------------------------
// StateTransitionSignal

/// Message emitted for every state transition of `S`.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct StateTransitionSignal<S: States> {
    /// Previously active state, if any.
    pub exited: Option<S>,
    /// Newly active state, if any.
    pub entered: Option<S>,
    /// Whether identity transitions should execute transition schedules.
    pub allow_same_state_transitions: bool,
}

// -----------------------------------------------------------------------------
// StateTransitionSystems

/// Ordered transition phases.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StateTransitionSystems {
    /// Apply queued state values to `State<S>` resources.
    Apply,
    /// Run exit hooks and schedules.
    Exit,
    /// Run transition hooks and schedules.
    Transition,
    /// Run enter hooks and schedules.
    Enter,
}

// -----------------------------------------------------------------------------
// StateTransitionSystems

/// System set that runs exit schedule(s) for state `S`.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
#[system_set(typed)]
pub struct ExitSchedules<S: States>(PhantomData<S>);

impl<S: States> Default for ExitSchedules<S> {
    #[inline(always)]
    fn default() -> Self {
        Self(PhantomData)
    }
}

/// System set that runs transition schedule(s) for state `S`.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
#[system_set(typed)]
pub struct TransitionSchedules<S: States>(PhantomData<S>);

impl<S: States> Default for TransitionSchedules<S> {
    #[inline(always)]
    fn default() -> Self {
        Self(PhantomData)
    }
}

/// System set that runs enter schedule(s) for state `S`.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
#[system_set(typed)]
pub struct EnterSchedules<S: States>(PhantomData<S>);

impl<S: States> Default for EnterSchedules<S> {
    #[inline(always)]
    fn default() -> Self {
        Self(PhantomData)
    }
}

/// System set that applies transitions for state `S`.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
#[system_set(typed)]
pub(crate) struct ApplyStateTransition<S: States>(PhantomData<S>);

impl<S: States> Default for ApplyStateTransition<S> {
    #[inline(always)]
    fn default() -> Self {
        Self(PhantomData)
    }
}

// -----------------------------------------------------------------------------
// StateTransitionSystems

// -----------------------------------------------------------------------------
// Systems

/// Applies one resolved transition for `S`.
///
/// This updates [`State<S>`] / [`PreviousState<S>`] resources and emits a
/// [`StateTransitionSignal<S>`] describing the change.
#[inline(never)]
pub(crate) fn internal_apply_state_transition<S: States>(
    mut commands: Commands,
    mut signal: MessageWriter<StateTransitionSignal<S>>,
    current_state: Option<ResMut<State<S>>>,
    previous_state: Option<ResMut<PreviousState<S>>>,
    new_state: Option<S>,
    allow_same_state_transitions: bool,
) {
    match new_state {
        Some(entered) => {
            match current_state {
                Some(mut state_resource) => {
                    let exited = match *state_resource == entered {
                        true => entered.clone(), // Avoid triggering change cetection.
                        false => mem::replace(&mut state_resource.0, entered.clone()),
                    };

                    signal.write(StateTransitionSignal {
                        exited: Some(exited.clone()),
                        entered: Some(entered),
                        allow_same_state_transitions,
                    });

                    if let Some(mut previous) = previous_state {
                        previous.0 = exited;
                    } else {
                        commands.insert_resource(PreviousState(exited));
                    }
                }
                None => {
                    commands.insert_resource(State::new(entered.clone()));

                    signal.write(StateTransitionSignal {
                        exited: None,
                        entered: Some(entered),
                        allow_same_state_transitions,
                    });

                    // When [`State<S>`] is initialized, there can be stale data in
                    // [`PreviousState<S>`] from a prior transition to `None`, so we remove it.
                    if previous_state.is_some() {
                        commands.remove_resource::<PreviousState<S>>();
                    }
                }
            }
        }
        None => {
            // We first remove the [`State<S>`] resource, and if one existed we compute dependent states,
            // send a transition event and run the `OnExit` schedule.
            if let Some(resource) = current_state {
                let exited = resource.get().clone();
                commands.remove_resource::<State<S>>();

                signal.write(StateTransitionSignal {
                    exited: Some(exited.clone()),
                    entered: None,
                    allow_same_state_transitions,
                });

                if let Some(mut previous_state) = previous_state {
                    previous_state.0 = exited;
                } else {
                    commands.insert_resource(PreviousState(exited));
                }
            }
        }
    }
}

/// Returns whether at least one transition signal for `S` exists in this run.
pub(crate) fn detect_transition<S: States>(
    mut reader: MessageReader<StateTransitionSignal<S>>,
) -> bool {
    reader.read().count() > 0 // O(1)
}

/// Reads and clones the latest transition signal for `S`, if any.
pub(crate) fn last_transition<S: States>(
    mut reader: MessageReader<StateTransitionSignal<S>>,
) -> Option<StateTransitionSignal<S>> {
    reader.read().last().cloned()
}

/// Runs the [`OnEnter`] schedule for the latest transition of `S`.
pub(crate) fn run_enter<S: States>(
    transition: In<Option<StateTransitionSignal<S>>>,
    world: &mut World,
) {
    let Some(transition) = transition.0 else {
        return;
    };

    if transition.entered == transition.exited && !transition.allow_same_state_transitions {
        return;
    }

    let Some(entered) = transition.entered else {
        return;
    };

    world.try_run_schedule(OnEnter(entered));
}

/// Runs the [`OnExit`] schedule for the latest transition of `S`.
pub(crate) fn run_exit<S: States>(
    transition: In<Option<StateTransitionSignal<S>>>,
    world: &mut World,
) {
    let Some(transition) = transition.0 else {
        return;
    };

    if transition.entered == transition.exited && !transition.allow_same_state_transitions {
        return;
    }

    let Some(exited) = transition.exited else {
        return;
    };

    world.try_run_schedule(OnExit(exited));
}

/// Runs the [`OnTransition`] schedule for the latest transition pair of `S`.
pub(crate) fn run_transition<S: States>(
    transition: In<Option<StateTransitionSignal<S>>>,
    world: &mut World,
) {
    let Some(transition) = transition.0 else {
        return;
    };

    let (Some(exited), Some(entered)) = (transition.exited, transition.entered) else {
        return;
    };

    world.try_run_schedule(OnTransition { exited, entered });
}
