# Time utilities and scheduling support

Provides time tracking resources and a fixed-timestep runner for `voker-app`:

- `Time<T>` generic resource with per-context delta / elapsed tracking,
- three built-in clock contexts: `Real`, `Virtual`, `Fixed`,
- `Timer` for one-shot and repeating countdowns,
- `Stopwatch` for elapsed-time measurement,
- `TimePlugin` that wires all clocks and the fixed-timestep loop into the schedule,
- `delayed` module for commands queued to fire after a duration.

## Core Types

| Type | Description |
|---|---|
| `Time` | Alias for `Time<()>`; mirrors virtual time and is the default clock for most systems. |
| `Time<Real>` | Wall-clock time; advances with `Instant::now()` and is never paused or scaled. |
| `Time<Virtual>` | Game time with configurable speed scale and pause support. |
| `Time<Fixed>` | Fixed-timestep clock; driven by `Time<Virtual>` overstep accumulation. |
| `Timer` | Counts down to zero; supports one-shot and repeating modes. |
| `Stopwatch` | Counts up; can be paused and reset. |
| `TimePlugin` | Adds all clock resources and time-update systems to an app. |
| `TimeUpdateStrategy` | Resource for overriding how `Real` time advances (useful in tests). |

## Quick Start

```rust
use voker_app::{App, Update};
use voker_time::prelude::*;
use voker_ecs::borrow::Res;

fn tick(time: Res<Time>) {
    let dt = time.delta_secs();
    // use dt for frame-rate-independent movement
    let _ = dt;
}

fn main() {
    App::new()
        .add_plugins(TimePlugin)
        .add_systems(Update, (), tick)
        .run();
}
```

## Clock Contexts

Read a specific context by parameterising `Time<T>`:

```rust
use voker_time::{Time, Real, Virtual, Fixed};
use voker_ecs::borrow::Res;

fn inspect_clocks(
    real:    Res<Time<Real>>,
    virt:    Res<Time<Virtual>>,
    fixed:   Res<Time<Fixed>>,
) {
    println!("wall  dt = {:.4}", real.delta_secs());
    println!("game  dt = {:.4}", virt.delta_secs());
    println!("fixed dt = {:.4}", fixed.delta_secs());  // always == fixed timestep
}
```

## Virtual Time — Pause and Speed Scaling

```rust
use voker_time::{Time, Virtual};
use voker_ecs::borrow::ResMut;

fn slow_motion(mut virt: ResMut<Time<Virtual>>) {
    virt.set_relative_speed(0.25);  // quarter speed
}

fn pause_game(mut virt: ResMut<Time<Virtual>>) {
    virt.pause();
}
```

## Fixed Timestep

The `FixedUpdate` schedule (and its siblings) runs zero or more times per frame, driven by
`Time<Fixed>`.  Configure the step rate via a resource:

```rust
use core::time::Duration;
use voker_time::{Time, Fixed};
use voker_ecs::borrow::ResMut;

fn configure_fixed(mut fixed: ResMut<Time<Fixed>>) {
    fixed.set_timestep_hz(60.0);                   // 60 Hz
    // or: fixed.set_timestep(Duration::from_millis(16));
}
```

Access the overstep fraction from rendering systems for sub-frame interpolation:

```rust
use voker_time::{Time, Fixed};
use voker_ecs::borrow::Res;

fn interpolate(fixed: Res<Time<Fixed>>) {
    let alpha = fixed.overstep_fraction();
    let _ = alpha;
}
```

## Timer

```rust
use core::time::Duration;
use voker_time::{Timer, TimerMode, Time};
use voker_ecs::borrow::{Res, ResMut};
use voker_ecs::resource::Resource;

#[derive(Resource)]
struct Cooldown(Timer);

fn tick_cooldown(time: Res<Time>, mut cooldown: ResMut<Cooldown>) {
    cooldown.0.tick(time.delta());
    if cooldown.0.just_finished() {
        // fire ability
    }
}
```

## Overriding the Time Source (Tests)

```rust
use core::time::Duration;
use voker_time::TimeUpdateStrategy;

// Advance by a fixed amount each frame — deterministic in tests.
app.world_mut().insert_resource(
    TimeUpdateStrategy::ManualDuration(Duration::from_millis(16))
);
```

## Feature Flags

- `std` (default): Enables `std`-backed platform time via `voker-os`.
