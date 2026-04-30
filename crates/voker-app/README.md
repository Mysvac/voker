# High-level application orchestration

Provides the runtime shell around ECS worlds and schedules:
- app lifecycle control,
- plugin registration and build flow,
- main/fixed schedule pipelines,
- sub-app composition and extraction/update routing.

## Core Types

- `App`: Primary application entry point.
- `SubApp`: A secondary app container with its own world/schedules.
- `Plugin`: Unit of app composition.
- `PluginGroup`: Ordered plugin bundles.
- `MainSchedulePlugin`: Installs default main/fixed schedule topology.
- `ScheduleRunnerPlugin`: Configurable run loop behavior.
- `TaskPoolPlugin`: Task pool initialization/configuration.

## Default Schedule Labels

Main pipeline labels:
- `PreStartup`, `Startup`, `PostStartup`
- `First`, `PreUpdate`, `Update`, `SpawnScene`, `PostUpdate`, `Last`

Fixed-timestep pipeline labels:
- `FixedFirst`, `FixedPreUpdate`, `FixedUpdate`, `FixedPostUpdate`, `FixedLast`

Root schedule labels:
- `Main`
- `FixedMain`

## Quick Start

```rust
use voker_app::{App, Startup, Update};

fn setup() {
    // one-time startup work
}

fn tick() {
    // per-frame work
}

fn main() {
    App::new()
        .add_systems(Startup, (), setup)
        .add_systems(Update, (), tick)
        .run();
}
```

## Plugin Workflow

A typical pattern is to expose one plugin per feature module:

```rust
use voker_app::{App, Plugin};

struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        // register resources/messages/systems here
        let _ = app;
    }
}
```

Then compose in app construction:

```rust
# use voker_app::{App, Plugin};
# struct GameplayPlugin;
# impl Plugin for GameplayPlugin { fn build(&self, _: &mut App) {} }
let mut app = App::new();
app.add_plugins(GameplayPlugin);
```

## SubApp Model

`App` owns one main sub-app plus optional labeled sub-apps.

On each `App::update()`:
1. the main schedule runs,
2. each registered sub-app performs extraction from the main world,
3. each sub-app updates its own schedules,
4. tracker state is cleared.

This enables domain separation (for example render/logic/tooling pipelines) while keeping a unified top-level control flow.

## Feature Flags

- `std` (default): Enables standard runtime integrations, including Ctrl+C handling support where available.
