use core::fmt::Debug;
use core::hash::Hash;

use voker_ecs::borrow::{Res, ResMut};
use voker_ecs::command::Commands;
use voker_ecs::message::{MessageReader, MessageWriter};
use voker_ecs::schedule::{InternedSystemSet, SystemSet};
use voker_ecs::schedule::{IntoSystemConfig, Schedule};
use voker_ecs::system::IntoSystem;

use super::ApplyStateTransition;
use super::{EnterSchedules, ExitSchedules, TransitionSchedules};
use super::{ManualStates, PreviousState, State, States};
use super::{NextState, OnEnter, OnExit, OnTransition};
use super::{StateTransitionSignal, StateTransitionSystems};
use super::{detect_transition, internal_apply_state_transition};
use super::{last_transition, run_enter, run_exit, run_transition};

// -----------------------------------------------------------------------------
// StateSet

mod sealed {
    pub trait Sealed {}
}

/// A source-state set for derived/sub-state computation.
///
/// Implementations exist for a single state, `Option<State>`, and tuples.
pub trait StateSet: sealed::Sealed {
    /// Aggregated dependency depth of this state source set.
    const STATE_SET_DEPENDENCY_DEPTH: usize;

    /// Registers systems required to drive derived-state transitions for `T`.
    fn register_derived_state_for<T: DerivedStates<SourceStates = Self>>(schedule: &mut Schedule);

    /// Registers systems required to drive sub-state transitions for `T`.
    fn register_sub_state_for<T: SubStates<SourceStates = Self>>(schedule: &mut Schedule);
}

// -----------------------------------------------------------------------------
// DerivedStates

/// A state type computed from one or more source states.
pub trait DerivedStates: 'static + Send + Sync + Clone + Eq + Hash + Debug {
    /// Source state set used to compute this state.
    type SourceStates: StateSet;

    /// Whether identity transitions should be allowed by default.
    const ALLOW_SAME_STATE_TRANSITIONS: bool = true;

    /// Automatically infer the current state based on the source state set.
    ///
    /// If None is returned, the status will be deleted.
    fn derive(sources: Self::SourceStates) -> Option<Self>;

    /// Registers this derived state into a transition schedule.
    #[inline]
    fn register_derived_state(schedule: &mut Schedule) {
        Self::SourceStates::register_derived_state_for::<Self>(schedule)
    }
}

impl<S: DerivedStates> States for S {
    const DEPENDENCY_DEPTH: usize = S::SourceStates::STATE_SET_DEPENDENCY_DEPTH + 1;
}

// -----------------------------------------------------------------------------
// SubStates

/// A manually mutable state whose existence depends on source states.
pub trait SubStates: States + ManualStates {
    /// Source state set controlling this sub-state lifecycle.
    type SourceStates: StateSet;

    /// Determine whether this state should exist.
    ///
    /// - Return `None`: Sub-states should not exist and should be
    ///   deleted if not manually set. Even if set manually, it may
    ///   be deleted in the next frame.
    /// - Return `Some(default)`: Sub-state should exist and return
    ///   a default value. If the current state does not exist and
    ///   no value is manually provided, this default value will be used.
    fn should_exist(sources: Self::SourceStates) -> Option<Self>;

    /// Registers this sub-state into a transition schedule.
    #[inline]
    fn register_sub_state(schedule: &mut Schedule) {
        Self::SourceStates::register_sub_state_for::<Self>(schedule)
    }
}

// -----------------------------------------------------------------------------
// StateSetItem

fn apply_of<S: States>() -> InternedSystemSet {
    ApplyStateTransition::<S>::default().intern()
}

fn enter_of<S: States>() -> InternedSystemSet {
    EnterSchedules::<S>::default().intern()
}

fn trans_of<S: States>() -> InternedSystemSet {
    TransitionSchedules::<S>::default().intern()
}

fn exit_of<S: States>() -> InternedSystemSet {
    ExitSchedules::<S>::default().intern()
}

trait StateSetItem: Sized + 'static {
    type RawState: States;

    const DEPENDENCY_DEPTH: usize;

    fn clone_raw_state(wrapped: Option<&State<Self::RawState>>) -> Option<Self>;
}

impl<S: States> StateSetItem for S {
    type RawState = Self;

    const DEPENDENCY_DEPTH: usize = <S as States>::DEPENDENCY_DEPTH;

    fn clone_raw_state(wrapped: Option<&State<Self::RawState>>) -> Option<Self> {
        wrapped.map(|v| v.0.clone())
    }
}

impl<S: States> StateSetItem for Option<S> {
    type RawState = S;

    const DEPENDENCY_DEPTH: usize = <S as States>::DEPENDENCY_DEPTH;

    fn clone_raw_state(wrapped: Option<&State<Self::RawState>>) -> Option<Self> {
        Some(wrapped.map(|v| v.0.clone()))
    }
}

// -----------------------------------------------------------------------------
// StateSet Implementation

impl<S: StateSetItem> sealed::Sealed for S {}

impl<S: StateSetItem> StateSet for S {
    const STATE_SET_DEPENDENCY_DEPTH: usize = S::DEPENDENCY_DEPTH;

    fn register_derived_state_for<T: DerivedStates<SourceStates = Self>>(schedule: &mut Schedule) {
        fn apply_state_transition<S, T>(
            commands: Commands,
            signal: MessageWriter<StateTransitionSignal<T>>,
            mut source_changed: MessageReader<StateTransitionSignal<S::RawState>>,
            current_state: Option<ResMut<State<T>>>,
            previous_state: Option<ResMut<PreviousState<T>>>,
            source_state: Option<Res<State<S::RawState>>>,
        ) where
            S: StateSetItem,
            T: DerivedStates<SourceStates = S>,
        {
            if source_changed.is_empty() {
                return;
            }
            source_changed.clear();

            let wrapped = source_state.as_deref();
            let new_state = S::clone_raw_state(wrapped).and_then(T::derive);

            internal_apply_state_transition(
                commands,
                signal,
                current_state,
                previous_state,
                new_state,
                T::ALLOW_SAME_STATE_TRANSITIONS,
            );
        }

        let apply = ApplyStateTransition::<T>::default().intern();
        let enter = EnterSchedules::<T>::default().intern();
        let trans = TransitionSchedules::<T>::default().intern();
        let exit = ExitSchedules::<T>::default().intern();

        schedule
            .add_systems(apply_state_transition::<S, T>.in_set(apply))
            .add_systems(
                last_transition::<T>
                    .pipe(run_enter::<T>)
                    .run_if(detect_transition::<T>.mark::<OnEnter<T>>())
                    .in_set(enter),
            )
            .add_systems(
                last_transition::<T>
                    .pipe(run_transition::<T>)
                    .run_if(detect_transition::<T>.mark::<OnTransition<T>>())
                    .in_set(trans),
            )
            .add_systems(
                last_transition::<T>
                    .pipe(run_exit::<T>)
                    .run_if(detect_transition::<T>.mark::<OnExit<T>>())
                    .in_set(exit),
            );

        schedule
            .config((apply.begin(), apply.end()).in_set(StateTransitionSystems::Apply))
            .config((exit.begin(), exit.end()).in_set(StateTransitionSystems::Exit))
            .config((trans.begin(), trans.end()).in_set(StateTransitionSystems::Transition))
            .config((enter.begin(), enter.end()).in_set(StateTransitionSystems::Enter))
            .config(apply.begin().after_set(apply_of::<S::RawState>()))
            .config(exit.begin().after_set(exit_of::<S::RawState>()))
            .config(trans.begin().after_set(trans_of::<S::RawState>()))
            .config(enter.begin().after_set(enter_of::<S::RawState>()));
    }

    fn register_sub_state_for<T: SubStates<SourceStates = Self>>(schedule: &mut Schedule) {
        // | parent changed | next state | already exists | should exist | what happens                     |
        // | -------------- | ---------- | -------------- | ------------ | -------------------------------- |
        // | false          | false      | false          | -            | -                                |
        // | false          | false      | true           | -            | -                                |
        // | false          | true       | false          | false        | -                                |
        // | true           | false      | false          | false        | -                                |
        // | true           | true       | false          | false        | -                                |
        // | true           | false      | true           | false        | Some(current) -> None            |
        // | true           | true       | true           | false        | Some(current) -> None            |
        // | true           | false      | false          | true         | None -> Some(default)            |
        // | true           | true       | false          | true         | None -> Some(next)               |
        // | true           | true       | true           | true         | Some(current) -> Some(next)      |
        // | false          | true       | true           | true         | Some(current) -> Some(next)      |
        // | true           | false      | true           | true         | Some(current) -> Some(current)   |
        fn apply_state_transition<S, T>(
            commands: Commands,
            signal: MessageWriter<StateTransitionSignal<T>>,
            mut source_changed: MessageReader<StateTransitionSignal<S::RawState>>,
            current_state: Option<ResMut<State<T>>>,
            previous_state: Option<ResMut<PreviousState<T>>>,
            next_state: Option<ResMut<NextState<T>>>,
            source_state: Option<Res<State<S::RawState>>>,
        ) where
            S: StateSetItem,
            T: SubStates<SourceStates = S>,
        {
            let source_changed = source_changed.read().last().is_some();
            let next_state = next_state.and_then(NextState::take);

            if !source_changed && next_state.is_none() {
                return;
            }

            let current = current_state.as_ref().map(|s| s.get()).cloned();

            let default_state = if source_changed {
                let wrapped = source_state.as_deref();
                S::clone_raw_state(wrapped).and_then(T::should_exist)
            } else {
                None
            };

            let allow_same_state_transitions = next_state
                .as_ref()
                .map(|(_, allow_same)| *allow_same)
                .unwrap_or_default();

            let new_state = next_state.map(|(next, _)| next).or(current).or(default_state);

            internal_apply_state_transition(
                commands,
                signal,
                current_state,
                previous_state,
                new_state,
                allow_same_state_transitions,
            );
        }

        let apply = ApplyStateTransition::<T>::default().intern();
        let enter = EnterSchedules::<T>::default().intern();
        let trans = TransitionSchedules::<T>::default().intern();
        let exit = ExitSchedules::<T>::default().intern();

        schedule
            .add_systems(apply_state_transition::<S, T>.in_set(apply))
            .add_systems(
                last_transition::<T>
                    .pipe(run_exit::<T>)
                    .run_if(detect_transition::<T>.mark::<OnExit<T>>())
                    .in_set(enter),
            )
            .add_systems(
                last_transition::<T>
                    .pipe(run_transition::<T>)
                    .run_if(detect_transition::<T>.mark::<OnTransition<T>>())
                    .in_set(trans),
            )
            .add_systems(
                last_transition::<T>
                    .pipe(run_enter::<T>)
                    .run_if(detect_transition::<T>.mark::<OnEnter<T>>())
                    .in_set(exit),
            );

        schedule
            .config((apply.begin(), apply.end()).in_set(StateTransitionSystems::Apply))
            .config((exit.begin(), exit.end()).in_set(StateTransitionSystems::Exit))
            .config((trans.begin(), trans.end()).in_set(StateTransitionSystems::Transition))
            .config((enter.begin(), enter.end()).in_set(StateTransitionSystems::Enter))
            .config(apply.begin().after_set(apply_of::<S::RawState>()))
            .config(exit.begin().after_set(exit_of::<S::RawState>()))
            .config(trans.begin().after_set(trans_of::<S::RawState>()))
            .config(enter.begin().after_set(enter_of::<S::RawState>()));
    }
}

macro_rules! impl_state_set_sealed_tuples {
    (0: []) => {};
    ($num:literal : [$($index:tt : $p:ident),+]) => {
        impl<$($p: StateSetItem),*> sealed::Sealed for ($($p,)*) {}

        impl<$($p: StateSetItem),*> StateSet for ($($p,)*) {
            const STATE_SET_DEPENDENCY_DEPTH: usize = 0 $(+ <$p as StateSet>::STATE_SET_DEPENDENCY_DEPTH)*;

            fn register_derived_state_for<T: DerivedStates<SourceStates = Self>>(schedule: &mut Schedule) {
                fn apply_state_transition<$($p,)* T>(
                    commands: Commands,
                    signal: MessageWriter<StateTransitionSignal<T>>,
                    mut sources_changed: ( $(MessageReader<StateTransitionSignal<$p::RawState>>,)* ),
                    current_state: Option<ResMut<State<T>>>,
                    previous_state: Option<ResMut<PreviousState<T>>>,
                    sources_state: ( $(Option<Res<State<$p::RawState>>>,)* ),
                )
                where
                    $($p: StateSetItem,)*
                    T: DerivedStates<SourceStates = ($($p,)*)>,
                {
                    if true $( && sources_changed.$index.is_empty() )* {
                        return;
                    }

                    $( sources_changed.$index.clear(); )*

                    let new_state = || -> Option<T> {
                        T::derive(( $(
                            <$p as StateSetItem>::clone_raw_state(sources_state.$index.as_deref())?,
                        )* ))
                    }();

                    internal_apply_state_transition(
                        commands,
                        signal,
                        current_state,
                        previous_state,
                        new_state,
                        <T as DerivedStates>::ALLOW_SAME_STATE_TRANSITIONS,
                    );
                }

                let apply = ApplyStateTransition::<T>::default().intern();
                let enter = EnterSchedules::<T>::default().intern();
                let trans = TransitionSchedules::<T>::default().intern();
                let exit = ExitSchedules::<T>::default().intern();

                schedule
                    .add_systems(apply_state_transition::<$($p,)* T>.in_set(apply))
                    .add_systems(
                        last_transition::<T>.pipe(run_enter::<T>)
                            .run_if(detect_transition::<T>.mark::<OnEnter<T>>())
                            .in_set(exit),
                    )
                    .add_systems(
                        last_transition::<T>.pipe(run_transition::<T>)
                            .run_if(detect_transition::<T>.mark::<OnTransition<T>>())
                            .in_set(trans),
                    )
                    .add_systems(
                        last_transition::<T>.pipe(run_exit::<T>)
                            .run_if(detect_transition::<T>.mark::<OnExit<T>>())
                            .in_set(enter),
                    );

                schedule
                    .config((apply.begin(), apply.end()).in_set(StateTransitionSystems::Apply))
                    .config((exit.begin(), exit.end()).in_set(StateTransitionSystems::Exit))
                    .config((trans.begin(), trans.end()).in_set(StateTransitionSystems::Transition))
                    .config((enter.begin(), enter.end()).in_set(StateTransitionSystems::Enter))
                    $(
                        .config(apply.begin().after_set(apply_of::<$p::RawState>()))
                        .config(exit.begin().after_set(exit_of::<$p::RawState>()))
                        .config(trans.begin().after_set(trans_of::<$p::RawState>()))
                        .config(enter.begin().after_set(enter_of::<$p::RawState>()))
                    )* ;
            }

            fn register_sub_state_for<T: SubStates<SourceStates = Self>>(schedule: &mut Schedule) {
                fn apply_state_transition<$($p,)* T>(
                    commands: Commands,
                    signal: MessageWriter<StateTransitionSignal<T>>,
                    mut sources_changed: ( $(MessageReader<StateTransitionSignal<$p::RawState>>,)* ),
                    current_state: Option<ResMut<State<T>>>,
                    previous_state: Option<ResMut<PreviousState<T>>>,
                    next_state: Option<ResMut<NextState<T>>>,
                    sources_state: ( $(Option<Res<State<$p::RawState>>>,)* ),
                )
                where
                    $($p: StateSetItem,)*
                    T: SubStates<SourceStates = ($($p,)*)>,
                {
                    let changed = false $( || sources_changed.$index.read().last().is_some() )*;
                    let next_state = next_state.and_then(NextState::take);

                    if !changed && next_state.is_none() {
                        return;
                    }

                    let current = current_state.as_ref().map(|s| s.get()).cloned();

                    let default_state = if changed {
                        || -> Option<T> {
                            T::should_exist(( $(
                                <$p as StateSetItem>::clone_raw_state(sources_state.$index.as_deref())?,
                            )* ))
                        } ()
                    } else {
                        None
                    };


                    let allow_same_state_transitions = next_state
                        .as_ref()
                        .map(|(_, allow_same)| *allow_same)
                        .unwrap_or_default();

                    let new_state = next_state
                        .map(|(next, _)| next)
                        .or(current)
                        .or(default_state);

                    internal_apply_state_transition(
                        commands,
                        signal,
                        current_state,
                        previous_state,
                        new_state,
                        allow_same_state_transitions,
                    );
                }


                let apply = ApplyStateTransition::<T>::default().intern();
                let enter = EnterSchedules::<T>::default().intern();
                let trans = TransitionSchedules::<T>::default().intern();
                let exit = ExitSchedules::<T>::default().intern();

                schedule
                    .add_systems(apply_state_transition::<$($p,)* T>.in_set(apply))
                    .add_systems(
                        last_transition::<T>.pipe(run_exit::<T>)
                            .run_if(detect_transition::<T>.mark::<OnExit<T>>())
                            .in_set(enter),
                    )
                    .add_systems(
                        last_transition::<T>.pipe(run_transition::<T>)
                            .run_if(detect_transition::<T>.mark::<OnTransition<T>>())
                            .in_set(trans),
                    )
                    .add_systems(
                        last_transition::<T>.pipe(run_enter::<T>)
                            .run_if(detect_transition::<T>.mark::<OnEnter<T>>())
                            .in_set(exit),
                    );

                schedule
                    .config((apply.begin(), apply.end()).in_set(StateTransitionSystems::Apply))
                    .config((exit.begin(), exit.end()).in_set(StateTransitionSystems::Exit))
                    .config((trans.begin(), trans.end()).in_set(StateTransitionSystems::Transition))
                    .config((enter.begin(), enter.end()).in_set(StateTransitionSystems::Enter))
                    $(
                        .config(apply.begin().after_set(apply_of::<$p::RawState>()))
                        .config(exit.begin().after_set(exit_of::<$p::RawState>()))
                        .config(trans.begin().after_set(trans_of::<$p::RawState>()))
                        .config(enter.begin().after_set(enter_of::<$p::RawState>()))
                    )* ;
            }
        }
    };
}

voker_utils::range_invoke!(impl_state_set_sealed_tuples, 12);
