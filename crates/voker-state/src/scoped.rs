use alloc::sync::Arc;

use voker_ecs::command::Commands;
use voker_ecs::derive::Component;
use voker_ecs::entity::Entity;
use voker_ecs::message::MessageReader;
use voker_ecs::query::Query;
use voker_reflect::Reflect;

use crate::state::{StateTransitionSignal, States};

// -----------------------------------------------------------------------------
// DespawnOnEnter

/// Despawn entities when the state transition enters this value.
#[derive(Reflect, Component, Clone)]
#[reflect(Component, Clone)]
pub struct DespawnOnEnter<S: States>(pub S);

impl<S: States + Default> Default for DespawnOnEnter<S> {
    fn default() -> Self {
        Self(S::default())
    }
}

// -----------------------------------------------------------------------------
// DespawnOnExit

/// Despawn entities when the state transition exits this value.
#[derive(Reflect, Component, Clone)]
#[reflect(Component, Clone)]
pub struct DespawnOnExit<S: States>(pub S);

impl<S: States + Default> Default for DespawnOnExit<S> {
    fn default() -> Self {
        Self(S::default())
    }
}

// -----------------------------------------------------------------------------
// DespawnWhen

/// Despawn entities when the custom predicate matches a state transition.
#[derive(Reflect, Component, Clone)]
#[reflect(Component, Clone)]
pub struct DespawnWhen<S: States> {
    /// Predicate executed against the latest transition signal.
    pub state_transition_evaluator:
        Arc<dyn Fn(&StateTransitionSignal<S>) -> bool + Send + Sync + 'static>,
}

impl<S: States> DespawnWhen<S> {
    /// Creates a new transition predicate.
    pub fn new(f: impl Fn(&StateTransitionSignal<S>) -> bool + Send + Sync + 'static) -> Self {
        Self {
            state_transition_evaluator: Arc::new(f),
        }
    }
}

// -----------------------------------------------------------------------------
// despawn_entities_on_enter_state

/// Despawn entities marked with [`DespawnOnEnter<S>`] for the entered state.
pub fn despawn_entities_on_enter_state<S: States>(
    mut commands: Commands,
    mut transitions: MessageReader<StateTransitionSignal<S>>,
    query: Query<(Entity, &DespawnOnEnter<S>)>,
) {
    let Some(transition) = transitions.read().last() else {
        return;
    };

    if transition.entered == transition.exited && !transition.allow_same_state_transitions {
        return;
    }

    let Some(entered) = &transition.entered else {
        return;
    };

    for (entity, marker) in &query {
        if marker.0 == *entered {
            commands.try_despawn(entity);
        }
    }
}

// -----------------------------------------------------------------------------
// despawn_entities_on_exit_state

/// Despawn entities marked with [`DespawnOnExit<S>`] for the exited state.
pub fn despawn_entities_on_exit_state<S: States>(
    mut commands: Commands,
    mut transitions: MessageReader<StateTransitionSignal<S>>,
    query: Query<(Entity, &DespawnOnExit<S>)>,
) {
    let Some(transition) = transitions.read().last() else {
        return;
    };

    if transition.entered == transition.exited && !transition.allow_same_state_transitions {
        return;
    }

    let Some(exited) = &transition.exited else {
        return;
    };

    for (entity, marker) in &query {
        if marker.0 == *exited {
            commands.try_despawn(entity);
        }
    }
}

// -----------------------------------------------------------------------------
// despawn_entities_when_state

/// Despawn entities marked with [`DespawnWhen<S>`] when predicate returns `true`.
pub fn despawn_entities_when_state<S: States>(
    mut commands: Commands,
    mut transitions: MessageReader<StateTransitionSignal<S>>,
    query: Query<(Entity, &DespawnWhen<S>)>,
) {
    let Some(transition) = transitions.read().last() else {
        return;
    };

    if transition.entered == transition.exited && !transition.allow_same_state_transitions {
        return;
    }

    for (entity, marker) in &query {
        if (marker.state_transition_evaluator)(transition) {
            commands.try_despawn(entity);
        }
    }
}
