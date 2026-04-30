# Performance diagnostics

Provides a lightweight, extensible diagnostics framework:

- `DiagnosticsStore` central registry keyed by `DiagnosticPath`,
- per-metric history with simple moving average (SMA) and exponential moving average (EMA),
- built-in plugins for frame count, entity count, and periodic log output,
- optional CPU/memory metrics via `sysinfo` (`sysinfo_plugin` feature).

## Core Types

| Type | Description |
|---|---|
| `DiagnosticsStore` | Resource holding all registered `Diagnostic` channels. |
| `Diagnostic` | A named timeline of `f64` samples with configurable history length and smoothing. |
| `DiagnosticPath` | Slash-separated string key (e.g. `"engine/frame_time"`). |
| `DiagnosticMeasurement` | A single `(Instant, f64)` sample. |
| `DiagnosticsPlugin` | Base plugin; initialises `DiagnosticsStore`. |

## Built-in Plugins

| Plugin | What it adds |
|---|---|
| `EntityCountPlugin` | Adds the `EntityCount` resource. |
| `EntityCountDiagnosticsPlugin` | Registers and updates the `entity_count` diagnostic each frame. |
| `FrameCountPlugin` | Adds the `FrameCount` resource. |
| `FrameCountDiagnosticsPlugin` | Registers and updates the `frame_count` diagnostic each frame. |
| `LogDiagnosticsPlugin` | Periodically prints all enabled diagnostics to the log. |
| `SystemInfoDiagnosticsPlugin` | CPU and memory usage (requires `sysinfo_plugin` feature). |

## Quick Start

```rust
use voker_app::App;
use voker_diagnostic::{
    DiagnosticsPlugin,
    EntityCountDiagnosticsPlugin,
    FrameCountDiagnosticsPlugin,
    LogDiagnosticsPlugin,
};

App::new()
    .add_plugins(DiagnosticsPlugin)
    .add_plugins(EntityCountDiagnosticsPlugin)
    .add_plugins(FrameCountDiagnosticsPlugin)
    .add_plugins(LogDiagnosticsPlugin::default())
    .run();
```

`LogDiagnosticsPlugin` prints a summary every second by default.

## Registering a Custom Diagnostic

```rust
use voker_diagnostic::{AppDiagnosticExt, Diagnostic, DiagnosticPath};

const MY_METRIC: DiagnosticPath = DiagnosticPath::new("my_system/processing_ms");

app.register_diagnostic(
    Diagnostic::new(MY_METRIC)
        .with_suffix("ms")
        .with_max_history_length(60),
);
```

## Recording Measurements

Inject `ResMut<DiagnosticsStore>` into any system:

```rust
use voker_diagnostic::{DiagnosticsStore, DiagnosticPath};
use voker_ecs::borrow::ResMut;

const PATH: DiagnosticPath = DiagnosticPath::new("my_system/processing_ms");

fn record(mut store: ResMut<DiagnosticsStore>) {
    store.add_measurement(&PATH, 3.7);
}
```

Use `add_measurement_with` for lazy evaluation — the closure is skipped when the
diagnostic is disabled:

```rust
store.add_measurement_with(&PATH, || compute_expensive_metric());
```

## Reading Aggregates

```rust
use voker_diagnostic::{DiagnosticsStore, DiagnosticPath};
use voker_ecs::borrow::Res;

const PATH: DiagnosticPath = DiagnosticPath::new("my_system/processing_ms");

fn read(store: Res<DiagnosticsStore>) {
    if let Some(d) = store.get(&PATH) {
        println!("latest  = {:?}", d.value());
        println!("average = {:?}", d.average());   // SMA
        println!("smoothed= {:?}", d.smoothed());  // EMA
    }
}
```

## Feature Flags

| Feature | Description |
|---|---|
| `std` (default) | Enables `std`-backed platform time and I/O. |
| `sysinfo_plugin` | Adds `SystemInfoDiagnosticsPlugin` for CPU and memory metrics (Linux, Windows, macOS, Android, FreeBSD). |
