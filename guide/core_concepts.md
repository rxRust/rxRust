# Core Concepts

While rxRust follows the [ReactiveX standard](http://reactivex.io), the implementation is adapted to Rust's unique ownership and type system.

## The Triad: Observable, Observer, Subscription

### 1. Observable (The Source)
In rxRust, an `Observable` is a lazy computation. Nothing happens until you subscribe.
*   **Rust Specific**: It owns its data. When you call an operator (e.g., `map`), the original observable is *consumed* (moved) into the new one.

### 2. Observer (The Consumer)
The logic that reacts to events (`next`, `error`, `complete`).
*   **Rust Specific**: You rarely implement the `Observer` trait manually. Instead, you pass a closure to `.subscribe(|v| ... )`.

### 3. Subscription (The Link)
The connection between Source and Consumer.

Unlike some other Rx implementations, an `Subscription` object itself **does not** automatically unsubscribe when it's dropped. This gives you explicit control over the subscription's lifecycle.

To enable **RAII** (Resource Acquisition Is Initialization) behavior—where the stream is automatically cancelled when a guard goes out of scope—you must explicitly call `unsubscribe_when_dropped()`:

*   **Explicit Control**: Call `subscription.unsubscribe()` manually.
*   **RAII (Automatic Unsubscribe)**: Use `subscription.unsubscribe_when_dropped()` to get a guard that unsubscribes on drop. You must hold this guard.

```rust,no_run
use rxrust::prelude::*;

// Example of explicit unsubscribe
let subscription = Local::interval(Duration::from_secs(1))
    .subscribe(|_| println!("Tick (explicit)"));
// Stream runs...
// To stop: subscription.unsubscribe();

// Example of RAII for automatic unsubscribe
{
    let sub = Local::interval(Duration::from_secs(1))
        .subscribe(|_| println!("Tick (RAII)"));
    let _guard = sub.unsubscribe_when_dropped();
    // Stream runs as long as _guard is in scope...
    // When _guard drops here, the stream is cancelled.
}
```

---

## Key Architectural Concepts

To adapt Reactive Programming to Rust's compile-time guarantees, rxRust introduces specific architectural patterns:

*   **[Context](core_concepts/context.md)**: Solves the *Thread-Safety vs Performance* dilemma by splitting the world into `Local` and `Shared`.
*   **[Scheduler](core_concepts/scheduler.md)**: Tightly integrated with Context to manage *Time* without boilerplate.
*   **[Type Erasure](core_concepts/type_erasure.md)**: Techniques (`box_it`, `impl Observable`) to manage Rust's complex iterator types.