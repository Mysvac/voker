# Rust Standard Library Abstraction Layer

## Architecture Overview

Rust's standard library is organized into three layers for maximum portability:

- **core**: Language core functionality, independent of OS and allocator
- **alloc**: Memory allocation APIs and common containers (`String`, `Vec`, etc.)
- **std**: OS-level APIs (files, threads, networking, etc.)

While Rust provides extensive [cross-platform support] through `std`,
it cannot cover every possible target—especially custom embedded systems or specialized game consoles.
This crate provides the necessary abstractions to bridge that gap.

[cross-platform support]: https://doc.rust-lang.org/nightly/rustc/platform-support.html

---

## Design Philosophy

We provide a thin abstraction layer over essential OS functionality, with multiple backend implementations selectable at compile time:

- **[`sync`]**: Synchronization primitives (`std::sync` compatibility)
- **[`time`]**: Time measurement APIs (`Instant` and `SystemTime`)
- **[`thread`]**: Thread utilities (`sleep` function only)
- **[`utils`]**: Some custom sync primitives and concurrent data structures

### Standard Backend (Default)
- **Direct re-exports** of `std` APIs with zero runtime overhead
- **Use case**: Most desktop, mobile, and web targets
- **Enabled by**: Default `std` feature

### Web Backend
- **Specialized implementation** for WebAssembly targets
- **Uses browser APIs** for time measurement and scheduling
- **Enabled when**: `target_arch = "wasm32"` and `features = ["web"]`

### No-Std Backend
- **For bare-metal** embedded systems and custom platforms
- **Provides spinlock-based** synchronization primitives
- **Time APIs require** manual configuration via `set_elapsed_getter()` functions
- **Memory allocation** requires a custom global allocator
- **Enabled by**: `no-std` feature (mutually exclusive with `std`)

---

## Feature Flags

### `std` (Enabled by Default)
- Uses standard library implementations
- Provides full OS-level functionality
- Required for most conventional platforms

### `web`
- WebAssembly-specific implementations (only on `wasm32` targets)
- Re-exports essential crates like `wasm-bindgen`
- Most functionality still utilizes std implementations
