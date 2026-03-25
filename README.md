# voker

A modular Rust engine foundation built as a workspace of focused crates.

This repository provides reusable building blocks for engine/runtime development:

| Crate | Purpose |
|---|---|
| voker | Umbrella crate that re-exports core modules (`app`, `ecs`, `reflect`, `task`, etc.). |
| voker-app | App/sub-app orchestration, plugin system, app scheduling entry points. |
| voker-ecs | Entity Component System runtime (world, schedule, query, resources, messages, commands). |
| voker-reflect | Runtime reflection, type registry, derive macros, reflection-based serde support. |
| voker-task | Lightweight task pool and async execution utilities (engine-oriented). |
| voker-os | OS abstraction layer for sync/time/thread utilities across platforms. |
| voker-utils | Common utility data structures and helpers used across the workspace. |
| voker-ptr | Low-level pointer wrappers and thin slice helpers for internal runtime code. |
| voker-cfg | Compile-time configuration macros and cfg alias helpers. |
| voker-path | Proc-macro path helpers and crate path resolution utilities. |
