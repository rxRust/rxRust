# Custom Scheduler & Runtimes

While rxRust provides robust default environments through `Local` (single-threaded) and `Shared` (multi-threaded) contexts, one of its most powerful features is the ability to adapt to specialized execution environments.

This guide explains how to inject custom schedulers into rxRust. This is the primary way to integrate rxRust with:
*   **Game Loops**: Synchronize time-based operators (`delay`, `interval`) with game ticks.
*   **GUI Event Loops**: Integrate with frameworks like GTK, Qt, or platform-specific loops.
*   **Test Environments**: Use virtual time for deterministic testing.
*   **Embedded Systems**: Run on bare-metal or RTOS environments.

## The "Context" vs. "Scheduler" Distinction

A common misconception is that you need to implement a new `Context` to change how tasks are executed. **This is almost never necessary.**

*   **Context (`Context` Trait)**: A complex container that manages **State** (the inner value), **Ownership Strategy** (Rc vs Arc), and **Type Propagation** (`With<T>`). Implementing this from scratch is difficult and involves complex Generic Associated Types (GATs).
*   **Scheduler (`Scheduler` Trait)**: The component responsible for *scheduling work* and *tracking time*. This is what you usually want to customize.

**The Recommended Pattern**: Use the existing `LocalCtx` or `SharedCtx` containers and simply **plug in** your custom Scheduler via a type alias.

## The Type Alias Pattern (Zero-Cost Injection)

rxRust is designed to allow compile-time injection of schedulers using Rust's type system. This incurs **zero runtime overhead** (no dynamic dispatch/vtable lookups for the scheduler itself).

### Step 1: Implement Your Scheduler

First, define your scheduler struct. You need to implement two traits:
1.  `SleepProvider`: Tells rxRust how to handle time (sleep/yield) in your environment. This automatically makes rxRust's internal tasks compatible with your scheduler.
2.  `Scheduler`: The main interface for scheduling tasks.

> **Note**: Your scheduler usually needs to implement `Default` to work seamlessly with the `ObservableFactory` methods (like `MyContext::of(...)`).

```rust,ignore
use rxrust::prelude::*;
use rxrust::scheduler::{Scheduler, Schedulable, TaskHandle, SleepProvider};
use std::time::Duration;

#[derive(Clone, Default)]
pub struct MyGameScheduler;

// 1. Implement SleepProvider
//    This allows rxRust's internal Task<S> to automatically run on your scheduler.
impl SleepProvider for MyGameScheduler {
    // The future type returned by sleep(). 
    // In a game engine, this might be a handle to a timer system.
    type SleepFuture = std::future::Ready<()>; 

    fn sleep(&self, _duration: Duration) -> Self::SleepFuture {
        // Implementation that waits for 'duration'
        std::future::ready(())
    }
}

// 2. Implement the Scheduler logic
//    We implement it for ANY Schedulable item (S), which covers both
//    rxRust Tasks (via SleepProvider) and native Futures.
impl<S> Scheduler<S> for MyGameScheduler
where
    S: Schedulable<MyGameScheduler> + 'static,
{
    fn schedule(&self, source: S, delay: Option<Duration>) -> TaskHandle {
        // Convert the source into a Future
        let future = source.into_future(self);
        
        // Push the future to your game loop's executor
        // global_game_loop::spawn(future);
        
        TaskHandle::finished() // Return a handle to allow cancellation
    }
}
```

### Step 2: Define the Context Alias

Now, create a type alias that binds `LocalCtx` (for single-threaded) or `SharedCtx` (for multi-threaded) to your new scheduler.

```rust,ignore
use rxrust::context::LocalCtx;

// This is your new "Context". Use it just like `Local`.
pub type GameRx<T> = LocalCtx<T, MyGameScheduler>;
```

### Step 3: Use It!

You can now use `GameRx` as the entry point for your reactive streams. All operators created from this chain will automatically use `MyGameScheduler` for time-based operations.

```rust,ignore
fn main() {
    // This looks exactly like standard rxRust code!
    GameRx::of(42)
        .delay(Duration::from_millis(100)) // Uses MyGameScheduler logic!
        .map(|v| v * 2)
        .subscribe(|v| println!("Value: {}", v));
}
```

## Why This Works: Context as an Environment Carrier

Under the hood, rxRust's `Context` acts as a dependency injection system. 

When you call `GameRx::of(42)`, you are utilizing the `ObservableFactory` trait (which `LocalCtx` implements) to create a struct:
```rust,ignore
LocalCtx {
    inner: Of(42),
    scheduler: MyGameScheduler::default(), // Automatically injected here
}
```

When you apply an operator like `.map(...)`, the `Context` trait's `With<T>` associated type ensures the scheduler type is preserved:

```rust,ignore
// Simplified view of what .map() does
fn map(self, f) -> LocalCtx<Map<...>, MyGameScheduler> {
    LocalCtx {
        inner: Map::new(self.inner, f),
        scheduler: self.scheduler, // Passes the SAME scheduler instance down
    }
}
```

This guarantees that a `delay` operator ten steps down the chain still has access to the correct `MyGameScheduler` without you ever passing it explicitly.

## When to Implement `Context` Manually?

Implementing the full `Context` trait is an advanced task reserved for scenarios where `LocalCtx` (Rc/RefCell) or `SharedCtx` (Arc/Mutex) are fundamentally incompatible with your requirements.

**Examples:**
*   **Bare-metal Embedded**: If you have no heap (`alloc`) and cannot use `Rc` or `Arc`.
*   **Specialized GC**: If you are integrating with a garbage-collected runtime (like a JS engine host) and need to use `Gc<T>` instead of standard reference counting.

If you find yourself in this situation, refer to `src/context.rs` and the `impl_context_for_container!` macro for the reference implementation. You will need to define:
1.  **Storage Strategy**: How inner values are stored (`Rc`, `Arc`, or custom).
2.  **Mutability Strategy**: How mutation is handled (`RefCell`, `Mutex`, or custom cells).

## Complete Example

For a fully runnable example that implements a logging scheduler and executes tasks, check out:
`examples/custom_scheduler.rs`

You can run it with:
```bash
cargo run --example custom_scheduler
```
