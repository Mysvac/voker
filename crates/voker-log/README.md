# Logging facilities and configuration

Provides a `tracing`-backed logging layer for voker applications:

- `LogPlugin` installs a process-wide subscriber and a `log`→`tracing` bridge,
- re-exports the standard `tracing` macros (`debug!`, `info!`, `warn!`, `error!`, `trace!`),
- `*_once!` variants emit each message at most once per process,
- automatic platform integrations: Android system log and iOS OSLog,
- composable via custom layers and a replaceable `fmt` layer.

## Core Types

- `LogPlugin`: plugin that sets up the global subscriber; add it once per process.
- `DEFAULT_FILTER`: default `EnvFilter` directives that reduce noise from common dependencies.
- `BoxedLayer`: type alias for an extra layer inserted before filtering.
- `BoxedFmtLayer`: type alias for a replacement `fmt` layer.

## Quick Start

```rust
use voker_app::App;
use voker_log::{LogPlugin, info};

fn main() {
    App::new()
        .add_plugins(LogPlugin::default())
        .run();

    info!("application started");
}
```

The default configuration logs at `INFO` and above.  Override it with `RUST_LOG`:

```sh
RUST_LOG=debug,wgpu=error cargo run
```

## Customising the Plugin

```rust
use voker_log::{LogPlugin, Level};

App::new().add_plugins(LogPlugin {
    level: Level::DEBUG,
    filter: "my_crate=trace,wgpu=error".into(),
    ..Default::default()
});
```

## Custom Layer

Attach an additional `tracing_subscriber` layer (for example a file sink or a remote
exporter) without replacing the default formatter:

```rust
use voker_log::{LogPlugin, BoxedLayer};
use voker_app::App;
use tracing_subscriber::Layer;

fn my_layer(_app: &mut App) -> Option<BoxedLayer> {
    // Return your layer here, e.g. a rolling file appender.
    None
}

App::new().add_plugins(LogPlugin {
    custom_layer: my_layer,
    ..Default::default()
});
```

## Log-Once Macros

The prelude exports `*_once!` variants that emit each call site at most once:

```rust
use voker_log::prelude::*;

fn expensive_path() {
    warn_once!("this code path has a known issue — logged only the first time");
}
```

## Feature Flags

| Feature | Description |
|---|---|
| `trace` | Installs `tracing-error` and prints a span trace alongside panics. |
| `trace_tracy_memory` | Replaces the global allocator with Tracy's profiled allocator for heap tracking. |
