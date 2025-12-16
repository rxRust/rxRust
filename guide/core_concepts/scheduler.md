# Scheduler

In Reactive Programming, a **Scheduler** controls *when* a task is executed.

## The rxRust Approach: Context-Bound Scheduling

A key difference in rxRust v1.0 is that the **Scheduler is implicitly bound to its Context**.

*   `Local` context -> `LocalScheduler`
*   `Shared` context -> `SharedScheduler`

This means operators like `delay`, `throttle`, and `debounce` automatically use the appropriate scheduling mechanism for your environment without needing explicit arguments.

### Explicit Scheduler Control (`xxx_with` methods)
For situations where you need to explicitly override the default scheduler, many time-based operators provide a `_with` suffix method.

```rust,no_run
use rxrust::prelude::*;

// Uses SharedScheduler explicitly
Shared::of(1)
    .delay_with(Duration::from_secs(1), SharedScheduler) 
    .subscribe(|v| println!("Delayed Shared Value: {}", v));
```

## Scheduler Types

### `LocalScheduler`
*   **Role**: Executes tasks on the current thread's event loop.
*   **Environment**:
    *   **WASM**: Wraps `setTimeout` / `requestAnimationFrame`.
    *   **Native**: Integrates with the local task runner (e.g., `tokio::task::LocalSet`).
*   **Key Benefit**: Does **not** require `Send`. This allows you to schedule tasks involving `Rc<RefCell<T>>` or DOM objects, avoiding the overhead of atomics and mutexes.

### `SharedScheduler`
*   **Role**: Executes tasks in a thread-safe manner, often allowing parallelism.
*   **Environment**:
    *   **Native**: Dispatches to a global thread pool (e.g., `tokio::spawn`). Requires `Send`.
    *   **WASM**: Falls back to `spawn_local` (since WASM threads are limited), but still enforces `Send` bounds to ensure portability.
*   **Key Benefit**: Enables utilizing multi-core processors for heavy computations.

## Scheduler as a Handle (Important!)

In rxRust, a `Scheduler` struct is essentially a **handle** or a **pointer** to an execution runtime. It is **not** the runtime itself.

*   **Instance-Agnostic**: Creating a new instance of `LocalScheduler` (via `LocalScheduler::default()`) **does not** create a new thread or event loop. All `LocalScheduler` instances in the same thread share the same underlying thread-local executor.
*   **Shared Runtime**: Similarly, all `SharedScheduler` instances dispatch tasks to the same global thread pool (e.g., Tokio's global runtime).

This design allows operators to cheaply create scheduler instances on the fly without performance penalties or resource leaks. When implementing your own scheduler, ensure it follows this pattern: your scheduler struct should be a lightweight handle to a shared, persistent runtime.

## Unified Task Management (`TaskHandle`)

In rxRust, the result of scheduling a task is a `TaskHandle`.
*   It implements **`Subscription`** (cancellable).
*   It implements **`Future`** (awaitable).

This unification simplifies resource management, allowing you to treat Rx subscriptions and Async futures uniformly.

## Switching Schedulers (`observe_on` and `subscribe_on`)

You can use specific operators to control where an Observable chain executes.

### `observe_on` (Where Observation Occurs)
`observe_on` changes the scheduler for **all downstream operations** from the point it's applied. It dictates *where the emissions are processed*.

### `subscribe_on` (Where Subscription Occurs)
`subscribe_on` changes the scheduler where the **entire Observable chain's subscription logic and source execution occurs**. It dictates *where the stream is set up and starts producing values*.

**Key Differences:**
*   `observe_on` affects the **consumer side** of the stream after it's been set up.
*   `subscribe_on` affects the **producer side** and the initial setup of the stream.

**Example:**

```rust,no_run
use rxrust::prelude::*;
use std::thread;

fn main() {
    println!("Main Thread: {:?}", thread::current().id());

    // This Observable will be subscribed to and execute on SharedScheduler
    let source = Shared::interval(Duration::from_millis(100));

    source
        .map(|v| {
            println!("Source execution on: {:?}", thread::current().id());
            v * 2
        })
        .subscribe_on(SharedScheduler) // The subscription and source will run on SharedScheduler
        .observe_on(LocalScheduler) // All emissions *after* this point will be observed on LocalScheduler
        .subscribe(|v| {
            println!("Observer on: {:?}", thread::current().id());
            println!("Value: {}", v);
        });
}
```

### Rules for Switching

If your stream is created in a `Local` context (and thus holds `Rc` or `!Send` data), you **cannot** simply `observe_on(SharedScheduler)` or `subscribe_on(SharedScheduler)` to move it to a background thread. The Rust compiler will prevent this because `Rc` and non-`Send` types cannot safely cross thread boundaries.

**Correct Pattern for Offloading Work:**

If you need to perform heavy computation on a background thread, you must ensure your data stream starts as `Shared` (or carries only `Send` data) from the beginning.

```rust,no_run
use rxrust::prelude::*;
use std::thread;

// Correct: Start with Shared context if you plan to use threads
Shared::from_iter(1..=1000)
    .map(|v| v * 2) // Runs on creating thread
    .observe_on(SharedScheduler) // Switch to thread pool for map/further ops
    .map(|v| {
        // Heavy computation here
        println!("Background: {:?}", thread::current().id());
        v
    })
    .subscribe(|_| {});
```

## Virtual Time for Testing (`TestScheduler`)

rxRust provides a `TestScheduler` to simulate time. This is essential for testing time-dependent logic (like `debounce`) instantly and deterministically.