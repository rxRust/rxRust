# Context

In rxRust v1.0, the **Context** is the foundation that defines the **execution environment** of a reactive stream.

It answers three key questions for the compiler and the runtime:
1.  **Memory Strategy**: How is state shared? (`Rc` vs `Arc`)
2.  **Scheduling**: How is time managed? (Main Loop vs Thread Pool)
3.  **Thread Safety**: Can this stream cross thread boundaries? (`!Send` vs `Send + Sync`)

## Why Context?

In Rust, generic libraries often face a dilemma:
*   Make everything `Send + Sync` (Thread-safe) -> Incurs overhead (`Arc<Mutex>`) which is wasteful for single-threaded environments like WASM or GUIs.
*   Make everything `!Send` (Local) -> Prevents usage in multi-threaded server environments.

rxRust solves this by introducing a generic `Context` trait. Instead of forcing a single implementation, rxRust provides **Unified Logic** that adapts to the **Context** you choose.

## Standard Contexts

rxRust provides two built-in contexts to cover the most common scenarios.

### 1. `Local` Context
*   **Environment**: Single-threaded (Main Thread, WASM, Embedded).
*   **Strategy**: Uses `Rc<RefCell<T>>` for sharing state.
*   **Scheduler**: `LocalScheduler` (runs on the current thread's loop).
*   **Constraint**: `!Send`. The compiler prevents these streams from moving to other threads.

### 2. `Shared` Context
*   **Environment**: Multi-threaded (Background workers, Servers).
*   **Strategy**: Uses `Arc<Mutex<T>>` for sharing state.
*   **Scheduler**: `SharedScheduler` (uses a thread pool).
*   **Constraint**: `Send + Sync`. Safe to move across threads.

## Important: Context ≠ Auto-Concurrency

Using `Shared` context does **not** automatically spawn threads for every operator. It simply **enables** thread safety guarantees so you *can* use concurrency.

*   By default, `Shared::of(1).map(...)` executes on the current thread, just with thread-safe primitives.
*   To actually execute on another thread, you must explicitly use **Scheduler Operators**:

```rust,no_run
use rxrust::prelude::*;
use std::thread;

// 1. Create in Shared context (Thread-Safe)
let stream = Shared::from_iter(1..=1000)
    .map(|v| v * 2);

// 2. Explicitly schedule subscription on a background thread
stream
    .subscribe_on(SharedScheduler) 
    .subscribe(|v| println!("Received on {:?}: {}", thread::current().id(), v));
```

## The "Factory" Pattern

Contexts acts as factories for creating Observables. This ensures that a stream starts with the correct environment settings from the very beginning.

```rust
use rxrust::prelude::*;

// Create a stream optimized for the Local environment
Local::of(1); 

// Create a stream ready for Multi-threaded environments
Shared::of(1);
```

## Custom Contexts (Extensibility)

The `Context` is a trait, not a hardcoded enum. This means rxRust is extensible.
Advanced users can implement their own Context to integrate with:
*   **Custom Async Runtimes**: Integrate rxRust with your own event loop or asynchronous runtime, bypassing default integrations like `tokio`.
*   **Game Engines**: Bind the Scheduler to the game's frame loop.
*   **Embedded Systems**: Bind to a custom hardware timer or interrupt controller.
*   **Frameworks**: Deep integration with frameworks like Tauri or Ribir.

See [Advanced Topics: Custom Context](../advanced/custom_context.md) for implementation details.
