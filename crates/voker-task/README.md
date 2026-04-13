# A refreshingly simple task executor for voker

This is a lightweight threadpool implementation with minimal dependencies, designed specifically
for voker as a lighter alternative to `rayon` for fork-join parallelism patterns.
The primary use case involves spawning tasks from a single thread and having that thread
await their completion. This library is game-oriented and makes no guarantees about
task fairness or execution order.

> The overall design is inspired by `bevy_tasks`, but we've implemented
> our own executors rather than relying on `async_executor`.

## Task Pool

The core structure is [`TaskPool`], which manages thread resources and automatically executes
submitted futures. Typically created on the main thread.

### Task Execution

We provide four distinct interfaces for different scenarios:

- **[`TaskPool::spawn`]**: Handles `'static + Send` tasks. Tasks are dispatched to the pool's
  `GlobalExecutor`, which distributes them to worker threads. Returns a `Task` handle without
  blocking the current thread. `Task` is a thin wrapper around [`async_task::Task`], which
  can be `await`ed for results, detached to run in the background, or canceled.

- **[`TaskPool::spawn_local`]**: Handles `'static + !Send` tasks. Tasks are assigned to the
  current thread's `LocalExecutor`, also returning a `Task` handle without blocking.
  Worker threads automatically tick their local executors, but the main thread's local
  executor requires manual `try_tick` calls to avoid blocking.

- **[`TaskPool::scope`]**: Handles `Send + !'static` tasks, blocking the current thread until
  all tasks complete. Tasks can be submitted either to the pool's global executor or the
  current thread's `ThreadExecutor`. The scope executor, driven by `Scope`, ensures tasks
  execute on the current thread (unlike the global executor's multi-thread distribution).

- **[`TaskPool::scope_with`]**: Handles `Send + !'static` tasks, primarily for
  dispatching tasks from the current thread to the main thread. This function can spawn
  tasks into any specified scope executor, not just the current thread's. However, `Scope`
  can only drive its own thread's scope executor and must await task completion.
  The target executor typically resides on the main thread, where game engines usually
  implement additional logic to process tasks from other threads.

### Predefined Task Pools

Three specialized task pools are provided for different workloads:

- **[`ComputeTaskPool`]**: For compute-intensive tasks expected to complete within a single frame.
- **[`AsyncComputeTaskPool`]**: For compute-intensive tasks that may span multiple frames.
- **[`IoTaskPool`]**: For IO-bound tasks involving potentially long waits.

Internally, all are `TaskPool` instances but can be configured with different worker counts
to optimize overall efficiency.

## Examples

### Spawn and await a `Send` task

```rust
use voker_task::{TaskPool, block_on};

let pool = TaskPool::new();
let task = pool.spawn(async { 21 * 2 });

assert_eq!(block_on(task), 42);
```

### Borrow stack data with `scope`

```rust
use voker_task::TaskPool;

let pool = TaskPool::new();
let values = [1_u32, 2, 3, 4];

let mut doubled = pool.scope(|scope| {
    for value in &values {
      scope.spawn(async move { *value * 2 });
    }
});

doubled.sort_unstable();
assert_eq!(doubled, vec![2, 4, 6, 8]);
```

### Drive a `!Send` task on the main thread

```rust
use core::cell::Cell;
use std::rc::Rc;
use voker_task::{TaskPool, block_on};

let pool = TaskPool::new();
let value = Rc::new(Cell::new(0));
let value_for_task = Rc::clone(&value);

let task = pool.spawn_local(async move {
    value_for_task.set(7);
    value_for_task.get()
});

let ticker = TaskPool::local_ticker();
while ticker.try_tick() {}

assert_eq!(block_on(task), 7);
assert_eq!(value.get(), 7);
```

## Platform Support

### `no_std` Support

Disable default features to enable `no_std` mode. `block_on` continuously polls until task completion.

### WASM Support

Enable the `web` feature when targeting WebAssembly to activate WASM-specific single-threaded mode.

## Single-Threaded Mode

In `no_std` or WebAssembly (WASM) environments, the library operates in single-threaded mode
with only a `ThreadExecutor`. All tasks execute on the current thread, blocking it during execution.

- In WASM, tasks use `wasm_bindgen_futures::spawn_local` under the hood and task handles are
  receiver-backed (`async_channel`). `detach` is effectively a no-op, and dropping the handle
  does not cancel the browser event-loop task.
- In `no_std` fallback mode, `spawn` and `spawn_local` actively tick the local executor and block
  until the local queue is drained. `scope` also executes tasks to completion on the current thread.

## Multi-Threaded Model

In standard (non-WASM) environments with `std` enabled, the library operates in multi-threaded
mode with two executor types:

- **`ThreadExecutor`**: Thread-local storage allowing tasks to be spawned from other threads but
  executed only on the owning thread. Requires manual ticking or `Scope`-driven automatic ticking.
- **`GlobalExecutor`**: Pool-level executor (one per `TaskPool`, not per thread) with a thread-safe
  task queue. Each worker thread has a `Worker` bound to its pool's global executor. Workers
  continuously execute tasks, maintaining local queues and supporting work-stealing from both
  the global queue and other workers' local queues. The main thread's `Worker` and `LocalExecutor`
  don't auto-execute or steal work.

## Feature Flags

### Default Features

- `std`: Enables standard library support and multi-threaded mode.

### Optional Features

- **`web`**: Enables WASM support, using `wasm_bindgen_futures` for the event loop. Implicitly
  requires standard library support but can be used without this crate's `std` feature.

- **`async_io`**: Available only in non-WASM `std` environments. Uses `async_io::block_on` as
  the blocking function for executors, which may improve efficiency if the project already uses
  `async_io`. (Defaults to `futures_lite::futures::block_on` in non-`no_std` environments.)

- **No dedicated `no_std` feature**: Disable all features to enable `no_std` mode with thread-local
  execution via `block_on`.
