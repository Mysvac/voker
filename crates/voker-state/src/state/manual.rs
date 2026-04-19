use voker_ecs::borrow::ResMut;
use voker_ecs::command::Commands;
use voker_ecs::message::MessageWriter;
use voker_ecs::prelude::SystemSet;
use voker_ecs::schedule::{IntoSystemConfig, Schedule};
use voker_ecs::system::IntoSystem;

use super::{ApplyStateTransition, StateTransitionSystems};
use super::{EnterSchedules, ExitSchedules, TransitionSchedules};
use super::{NextState, PreviousState, State};
use super::{OnEnter, OnExit, OnTransition};
use super::{StateTransitionSignal, States};
use super::{detect_transition, internal_apply_state_transition};
use super::{last_transition, run_enter, run_exit, run_transition};

// -----------------------------------------------------------------------------
// register_state

#[diagnostic::on_unimplemented(note = "consider annotating `{Self}` with `#[derive(States)]`")]
/// Marker trait for manually mutable state types.
///
/// This trait wires transition apply/exit/transition/enter systems for the
/// state type into the [`Schedule`].
pub trait ManualStates: States {
    /// Registers transition systems and transition-phase set boundaries.
    fn register_state(schedule: &mut Schedule) {
        let apply = ApplyStateTransition::<Self>::default().intern();
        let exit = ExitSchedules::<Self>::default().intern();
        let trans = TransitionSchedules::<Self>::default().intern();
        let enter = EnterSchedules::<Self>::default().intern();

        schedule
            .add_systems(apply_state_transition::<Self>.in_set(apply))
            .add_systems(
                last_transition::<Self>
                    .pipe(run_exit::<Self>)
                    .run_if(detect_transition::<Self>.mark::<OnExit<Self>>())
                    .in_set(exit),
            )
            .add_systems(
                last_transition::<Self>
                    .pipe(run_transition::<Self>)
                    .run_if(detect_transition::<Self>.mark::<OnTransition<Self>>())
                    .in_set(trans),
            )
            .add_systems(
                last_transition::<Self>
                    .pipe(run_enter::<Self>)
                    .run_if(detect_transition::<Self>.mark::<OnEnter<Self>>())
                    .in_set(enter),
            );

        schedule
            .config((apply.begin(), apply.end()).in_set(StateTransitionSystems::Apply))
            .config((exit.begin(), exit.end()).in_set(StateTransitionSystems::Exit))
            .config((trans.begin(), trans.end()).in_set(StateTransitionSystems::Transition))
            .config((enter.begin(), enter.end()).in_set(StateTransitionSystems::Enter));
    }
}

fn apply_state_transition<S: ManualStates>(
    signal: MessageWriter<StateTransitionSignal<S>>,
    commands: Commands,
    current_state: Option<ResMut<State<S>>>,
    previous_state: Option<ResMut<PreviousState<S>>>,
    next_state: Option<ResMut<NextState<S>>>,
) {
    let Some(next_state) = next_state else {
        return;
    };

    let Some(next_state) = NextState::take(next_state) else {
        return;
    };

    let (new_state, allow_same_state_transitions) = next_state;

    internal_apply_state_transition(
        commands,
        signal,
        current_state,
        previous_state,
        Some(new_state),
        allow_same_state_transitions,
    );
}
