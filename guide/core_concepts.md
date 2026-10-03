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

## Cancellation and terminal notifications

`complete` and `error` end notification delivery. They do not implicitly invoke
`unsubscribe()` on the source that sent the terminal notification. A combining
operator cancels other active inputs when it no longer needs them. For example,
`zip` completes when a completed input's buffer runs out, while `combine_latest`
can keep using the last value of a completed input. An input that completes
without a value makes `combine_latest` complete immediately.

Short-circuiting operators such as `take`, `take_while`, and `contains` explicitly
cancel their upstream. `take(0)` completes without subscribing to its source.
Because a source can emit synchronously before returning its subscription,
cancellation may be recorded first and applied as soon as that handle arrives.
Operators cannot invoke a cancellation handle before the source returns it;
synchronous sources should also check `Observer::is_closed()`.

Operator subscription handles implement `Subscription`; their concrete types may
include shared cancellation state. A scheduled source's `TaskHandle` can be
awaited directly, but that does not make every operator subscription a future.
Use completion notifications to observe a composed stream's natural termination.

`publish().ref_count()` owns a separate count of explicit subscription references.
It subscribes to the Subject, then reserves and starts a source connection if
needed. It disconnects when the last reference is explicitly released.
Connection generations prevent a disconnected source from notifying a later
connection's subscribers. Dropping an ordinary handle does not release its
reference; use explicit cancellation or `unsubscribe_when_dropped()`.

`ref_count` follows Subject's existing re-entrancy policy. Subscription changes
inside a Subject callback may take effect later; `ref_count` does not wait for
registration or defer connection startup. Cancelling inside a callback is
supported. If releasing the last reference and resubscribing in that callback
starts a source that emits synchronously, Subject rejects the re-entrant
emission. Schedule the resubscription explicitly after the callback to support
that feedback flow.

Subscription contexts retain their scheduler instance through transformations,
subscription, and connection management. Factories and explicit context
conversions may still select a default scheduler as documented by their APIs.
