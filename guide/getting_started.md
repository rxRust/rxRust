# Getting Started

## Installation

Add rxRust to your `Cargo.toml`.

```toml
[dependencies]
rxrust = "1.0.0-beta.11"
```

## Hello World

Here is a minimal example using the `Local` context, which is optimized for single-threaded environments (like WASM or UI main threads).

```rust
use rxrust::prelude::*;

fn main() {
    // 1. Create an Observable that emits a single value (42)
    Local::of(42)
        // 2. Transform the value
        .map(|v| v * 2)
        // 3. Subscribe to consume the value
        .subscribe(|v| println!("Value: {}", v));
}
```

## The Mental Model

RxRust programming revolves around three key components:

1.  **Observable (The Source)**: Represents a stream of events over time. It's lazy—nothing happens until you subscribe.
2.  **Operators (The Pipeline)**: Pure functions that transform, filter, or combine streams (e.g., `map`, `filter`, `merge`).
3.  **Subscription (The Link)**: Connects an Observer to an Observable, starting the execution.

### Managing Subscriptions

Unlike standard Iterators, Observables push data. You often need to manage the lifecycle of a subscription (e.g., to cancel a network request or stop a timer).

```rust
use rxrust::prelude::*;

fn main() {
    // A 'Subject' allows us to imperatively push values into a stream
    let mut subject = Local::subject();

    // Create a subscription
    let subscription = subject.clone()
        .map(|x: i32| x * 2)
        .subscribe(|v| println!("Got: {}", v));

    // Push values
    subject.next(1); // Output: Got: 2
    subject.next(2); // Output: Got: 4

    // Cancel the subscription
    subscription.unsubscribe();

    // This value will be ignored
    subject.next(3); 
}
```

## Next Steps

Now that you understand the basic flow, dive into the core concepts:

- **[Context & Threading](core_concepts/context.md)**: Learn when to use `Local` vs `Shared`.
- **[Operators](operators.md)**: Explore the rich vocabulary of stream transformations.