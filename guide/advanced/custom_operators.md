# Custom Operators

In rxRust v1.0, the best way to implement a custom operator is to build it as an **Environment-Agnostic Operator** starting from the `CoreObservable` trait.

Instead of hacking together `Local` or `Shared` specific implementations, rxRust allows you to write the logic **once** and have it automatically adapt to any environment (Single-threaded, Multi-threaded, WASM, etc.).

## The Architecture

To create a custom operator, you typically need four core components:
1. **The Operator Struct**: Holds the source observable and any parameters (e.g., `Map { source, func }`)
2. **The Observer Struct**: Wraps the downstream observer to intercept and transform events (e.g., `MapObserver { observer, func }`)
3. **`CoreObservable` Implementation**: The "glue" logic that connects the operator to the source, generic over the `Context`
4. **Integration**: How you expose the operator (either as an extension trait for external use or directly in the Observable trait for contributions)

## Walkthrough: Implementing `MapToString`

Let's implement a `MapToString` operator that calls `.to_string()` on every emitted item.

### Step 1: The Operator Struct

First, define a struct to represent your operator. It needs to hold the source observable.

```rust
use rxrust::prelude::*;
use rxrust::observable::{CoreObservable, ObservableType};
use rxrust::observer::Observer;
use rxrust::context::Context;

pub struct MapToString<S> {
    pub source: S,
}
```

### Step 2: The Observer Logic

Define an Observer wrapper that intercepts the `next` event. This is where your operator's business logic lives.

```rust
use rxrust::observer::Observer;

pub struct MapToStringObserver<O> {
    observer: O,
}

impl<O, Item, Err> Observer<Item, Err> for MapToStringObserver<O>
where
    O: Observer<String, Err>, // Downstream expects String
    Item: std::fmt::Display,  // Upstream provides Display-able items
{
    fn next(&mut self, v: Item) {
        // LOGIC HERE: Transform item to String
        let s = v.to_string();
        // Pass to downstream
        self.observer.next(s);
    }

    fn error(self, e: Err) {
        self.observer.error(e);
    }

    fn complete(self) {
        self.observer.complete();
    }

    fn is_closed(&self) -> bool {
        self.observer.is_closed()
    }
}
```

### Step 3: `ObservableType`

Tell rxRust what kind of items your operator produces.

```rust
use rxrust::prelude::*;

pub struct MapToString<S> {
    pub source: S,
}

impl<S> ObservableType for MapToString<S>
where
    S: ObservableType,
{
    // Our operator always produces Strings
    type Item<'a> = String where Self: 'a;
    type Err = S::Err;
}
```

### Step 4: `CoreObservable` (The Glue)

This is the most critical part. We implement `CoreObservable<C>` where `C` is a generic **Context**.

This implementation says: "If the Source `S` can accept a Context wrapping our `MapToStringObserver`, then `MapToString` is a valid observable."

```rust ignore
impl<S, C> CoreObservable<C> for MapToString<S>
where
    C: Context,
    // Constraint: The source must be able to work with our wrapped observer
    S: CoreObservable<C::With<MapToStringObserver<C::Inner>>>,
    // Constraint: The source items must be Display-able
    for<'a> S::Item<'a>: std::fmt::Display,
{
    type Unsub = S::Unsub;

    fn subscribe(self, context: C) -> Self::Unsub {
        // The context.transform method is the key.
        // It unwraps the downstream observer (context.inner),
        // lets you wrap it in your own observer (MapToStringObserver),
        // and repacks it into the same type of Context (Local/Shared).
        let wrapped_context = context.transform(|observer| MapToStringObserver { observer });

        // Subscribe the source to our wrapped context
        self.source.subscribe(wrapped_context)
    }
}
```

### Step 5: Integration

This is where the path splits depending on your use case:

#### 5.1 External Operators (For Your Project)

If you're implementing this operator for use in your own project, create an extension trait:

```rust,ignore
pub trait MapToStringOp: Observable {
    fn map_to_string(self) -> Self::With<MapToString<Self::Inner>>
    where
        for<'a> Self::Item<'a>: std::fmt::Display,
    {
        // self.transform is a helper on the Observable trait
        // that handles the Context wrapping/unwrapping for the operator struct itself.
        self.transform(|source| MapToString { source })
    }
}

// Blanket implementation for all Observables
impl<T: Observable> MapToStringOp for T {}
```

**Usage:**

```rust,ignore
use rxrust::prelude::*;

fn main() {
    // Works in Local Context
    Local::of(123)
        .map_to_string()
        .subscribe(|s| println!("Local String: {}", s));

    // Works in Shared Context
    Shared::of(456)
        .map_to_string()
        .subscribe(|s| println!("Shared String: {}", s));
}
```

#### 5.2 Contributing Operators to rxRust

If you're contributing this operator to the rxRust library itself, integrate directly:

1. **Create your operator file** in the appropriate location:
   - All operators are located in `src/ops/` as individual files
   - No subdirectories - flat organization
   - Example: `src/ops/map_to_string.rs`

2. **Add the method to the main `Observable` trait** in `src/observable.rs`:

```rust,ignore
// In src/observable.rs
pub trait Observable: Sized {
    // ... existing methods

    fn map_to_string(self) -> Self::With<ops::MapToString<Self::Inner>>
    where
        for<'a> Self::Item<'a>: std::fmt::Display,
    {
        self.transform(|source| ops::MapToString { source })
    }
}
```

3. **Ensure proper re-exports** in `src/ops/mod.rs` and `src/lib.rs`

**Requirements for Contributing:**

1. **Documentation**: Include comprehensive rustdoc comments explaining:
   - What the operator does
   - Parameter descriptions
   - Example usage
   - Performance considerations

2. **Tests**: Add comprehensive tests covering:
   - Basic functionality
   - Edge cases
   - Error handling
   - Both `Local` and `Shared` contexts

3. **Location**: All operators go in `src/ops/` as individual files (flat organization). Examples of existing files:
   - `map.rs`, `filter.rs`: For operators that transform/filter items
   - `zip.rs`, `merge.rs`: For operators that combine multiple observables
   - `retry.rs`, `map_err.rs`: For error handling operators
   - `scan.rs`, `reduce.rs`: For aggregation operators
   - `debounce.rs`, `throttle.rs`: For time-based operators

## Summary

By starting from `CoreObservable<C>`, you ensure your operator:
1. **Environment Agnostic**: Works for `Local` (WASM/UI) and `Shared` (Tokio/Thread) automatically
2. **Type Safe**: Leveraging Rust's trait system to propagate types
3. **Performant**: Zero-cost abstractions; the compiler optimizes the wrapper structs away

The first 4 steps are identical for both external operators and contributions. Only step 5 (integration) differs based on where the operator will live.