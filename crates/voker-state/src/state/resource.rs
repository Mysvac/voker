use core::ops::Deref;

use voker_ecs::borrow::ResMut;
use voker_ecs::reflect::ReflectResource;
use voker_ecs::resource::Resource;
use voker_ecs::world::{FromWorld, World};
use voker_reflect::Reflect;

use crate::state::{ManualStates, States};

// -----------------------------------------------------------------------------
// State

/// The current value for state type `S`.
#[derive(Reflect, Resource, Debug, PartialEq, Eq)]
#[reflect(Debug, PartialEq)]
#[type_data(ReflectResource)]
pub struct State<S: States>(pub(crate) S);

impl<S: States> State<S> {
    /// Creates a new state resource holding `value`.
    pub fn new(value: S) -> Self {
        Self(value)
    }

    /// Returns the current state value.
    pub fn get(&self) -> &S {
        &self.0
    }
}

impl<S: States + FromWorld> FromWorld for State<S> {
    fn from_world(world: &mut World) -> Self {
        Self(S::from_world(world))
    }
}

impl<S: States> PartialEq<S> for State<S> {
    fn eq(&self, other: &S) -> bool {
        self.get() == other
    }
}

impl<S: States> Deref for State<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

// -----------------------------------------------------------------------------
// PreviousState

/// The previous value for state type `S`.
#[derive(Reflect, Resource, Debug, Clone, PartialEq, Eq)]
#[reflect(Debug, Clone, PartialEq)]
#[type_data(ReflectResource)]
pub struct PreviousState<S: States>(pub(crate) S);

impl<S: States> PreviousState<S> {
    /// Get the previous state.
    pub fn get(&self) -> &S {
        &self.0
    }
}

impl<S: States> PartialEq<S> for PreviousState<S> {
    fn eq(&self, other: &S) -> bool {
        self.get() == other
    }
}

impl<S: States> Deref for PreviousState<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// -----------------------------------------------------------------------------
// NextState

/// The queued transition for state type `S`.
#[derive(Reflect, Resource, Debug, Clone, Default)]
#[reflect(Default, Debug, Clone)]
#[type_data(ReflectResource)]
pub enum NextState<S: ManualStates> {
    /// No transition is queued.
    #[default]
    Unchanged,
    /// Transition to this value and allow identity transitions.
    Pending(S),
    /// Transition to this value only when different from current state.
    PendingIfNeq(S),
}

impl<S: ManualStates> NextState<S> {
    /// Queues a transition and allows identity transitions.
    pub fn set(&mut self, next: S) {
        *self = Self::Pending(next);
    }

    /// Queues a transition only when not equal to current queued `Pending` value.
    pub fn set_if_neq(&mut self, next: S) {
        if !matches!(self, Self::Pending(s) if s == &next) {
            *self = Self::PendingIfNeq(next);
        }
    }

    /// Clears any queued transition.
    pub fn reset(&mut self) {
        *self = Self::Unchanged;
    }

    /// Takes the queued state transition without triggering unnecessary
    /// change-detection churn when unchanged.
    ///
    /// Returns `(next_state, allow_same_state_transitions)` when a transition
    /// is present.
    pub(crate) fn take(this: ResMut<Self>) -> Option<(S, bool)> {
        // Avoid triggering change detection.
        let read_only: &Self = this.as_ref();
        if matches!(read_only, Self::Unchanged) {
            return None;
        }

        match core::mem::take::<Self>(this.into_inner()) {
            Self::Pending(next) => Some((next, true)),
            Self::PendingIfNeq(next) => Some((next, false)),
            Self::Unchanged => {
                core::hint::cold_path();
                None
            }
        }
    }
}
