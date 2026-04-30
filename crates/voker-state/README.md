# Finite state machine

Provides a type-safe, schedule-integrated finite state machine layer on top of `voker-app`:

- `States` and `SubStates` trait-based state types with derive macros,
- `OnEnter(S)` / `OnExit(S)` / `OnTransition { from, to }` schedule hooks,
- `State<S>`, `NextState<S>`, `PreviousState<S>` resources,
- run conditions: `in_state`, `state_changed`, `state_exists`,
- automatic entity cleanup via `DespawnOnEnter`, `DespawnOnExit`, `DespawnWhen`.

## Core Types

- `States`: marker trait all state enums must implement (auto-derived with `#[derive(States)]`).
- `SubStates`: a state that only exists while a parent state holds a specific value.
- `DerivedStates` / `ManualStates`: advanced derivation strategies.
- `State<S>`: resource holding the current state value.
- `NextState<S>`: resource used to queue a transition.
- `PreviousState<S>`: resource holding the state value from the previous tick.
- `OnEnter(S)` / `OnExit(S)` / `OnTransition`: schedule labels fired around transitions.
- `StateTransitionSignal<S>`: message broadcast when a transition fires.
- `StatesPlugin`: plugin that wires state transition systems into the main schedule.
- `AppStatesExt`: extension trait on `App` for state registration.

## Quick Start

```rust
use voker_app::{App, Update};
use voker_state::prelude::*;

#[derive(States, Default, Clone, PartialEq, Eq, Hash, Debug)]
enum GameState {
    #[default]
    Menu,
    Playing,
    Paused,
}

fn main() {
    App::new()
        .add_plugins(StatesPlugin)
        .init_state::<GameState>()
        .add_systems(OnEnter(GameState::Playing), (), setup_level)
        .add_systems(OnExit(GameState::Playing),  (), teardown_level)
        .add_systems(Update, in_state(GameState::Playing), game_tick)
        .run();
}

fn setup_level()   {}
fn teardown_level() {}
fn game_tick()     {}
```

## SubStates

`SubStates` only exist while the parent state has a specific value.

```rust
use voker_state::prelude::*;
use voker_state::derive::SubStates;

#[derive(States, Default, Clone, PartialEq, Eq, Hash, Debug)]
enum AppState { #[default] Loading, InGame }

#[derive(SubStates, Default, Clone, PartialEq, Eq, Hash, Debug)]
#[source(AppState = AppState::InGame)]
enum InGameState { #[default] Running, Paused }
```

Register with `App::add_sub_state::<InGameState>()`.  The sub-state resource is
inserted and removed automatically as the parent transitions.

## Scoped Entity Cleanup

Attach any of these components to an entity to despawn it on a state transition:

```rust
use voker_state::prelude::*;

// Despawn when GameState::Menu is entered.
commands.spawn(DespawnOnEnter(GameState::Menu));

// Despawn when GameState::Playing is exited.
commands.spawn(DespawnOnExit(GameState::Playing));

// Despawn on any custom transition predicate.
commands.spawn(DespawnWhen::new(|sig: &StateTransitionSignal<GameState>| {
    sig.entered == Some(GameState::Paused)
}));
```

## Run Conditions

```rust
use voker_state::prelude::*;

// Only run while in a specific state.
app.add_systems(Update, in_state(GameState::Playing), my_system);

// Run once when the state changes (regardless of direction).
app.add_systems(Update, state_changed::<GameState>(), on_state_changed);

// Run only while the state resource exists (useful with SubStates).
app.add_systems(Update, state_exists::<GameState>(), my_system);
```

## Requesting Transitions

Queue a transition from any system that has `Commands` or `ResMut<NextState<S>>`:

```rust
use voker_state::prelude::*;

fn pause(mut commands: Commands) {
    commands.set_state(GameState::Paused);
}
```
