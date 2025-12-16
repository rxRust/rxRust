# Refactoring Scheme 2: Outer Wrapper Pattern

## Overview
This scheme separates the core logic (subscription) from the API surface (operators). It introduces two distinct wrapper types, `Local<T>` and `Shared<T>`, which serve as the only entry points for operators. This enforces strict type isolation between single-threaded and multi-threaded workflows.

## Core Trait Definition

The core trait is stripped down to the bare minimum.

```rust
// Renamed from Observable to InnerObservable (or ObservableCore)
pub trait ObservableCore {
    type Item;
    type Err;
    type Unsub: Subscription;

    fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
    where O: Observer<Self::Item, Self::Err>;
}
```

## Wrapper Definitions

```rust
// Single-threaded Wrapper
#[derive(Clone)]
pub struct Local<S>(pub S);

// Multi-threaded Wrapper
#[derive(Clone)]
pub struct Shared<S>(pub S);
```

## Implementation Strategy

### 1. Macros for Code Generation
Since `Local` and `Shared` expose the same operators (map, filter, etc.) but with different trait bounds (`FnMut` vs `FnMut + Send + Sync`), we use a macro to generate the implementations.

```rust
macro_rules! impl_observable_api {
    ($wrapper:ident, $trait_bound:path, $closure_bound:tt) => {
        impl<S> $wrapper<S> where S: $trait_bound {
            
            pub fn map<B, F>(self, f: F) -> $wrapper<MapOp<S, F>>
            where F: FnMut(S::Item) -> B + $closure_bound 
            {
                $wrapper(MapOp::new(self.0, f))
            }

            pub fn filter<F>(self, f: F) -> $wrapper<FilterOp<S, F>>
            where F: FnMut(&S::Item) -> bool + $closure_bound
            {
                $wrapper(FilterOp::new(self.0, f))
            }
            
            // ... other operators ...
        }
    }
}

// Generate Local API (No Send/Sync required)
impl_observable_api!(Local, ObservableCore, ());

// Generate Shared API (Send + Sync required)
impl_observable_api!(Shared, (ObservableCore + Send + Sync), (Send + Sync));
```

### 2. Specific Implementations
Operators that behave differently (like `merge`) are implemented manually.

```rust
impl<S: ObservableCore> Local<S> {
    pub fn merge<O>(self, other: Local<O>) -> Local<MergeOp<S, O, LocalPolicy>> {
        Local(MergeOp::new(self.0, other.0))
    }
}

impl<S: ObservableCore + Send + Sync> Shared<S> {
    pub fn merge<O>(self, other: Shared<O>) -> Shared<MergeOp<S, O, SharedPolicy>> {
        Shared(MergeOp::new(self.0, other.0))
    }
}
```

## Usage Example

```rust
// Local
rx::of(1)               // Returns Local<Of>
    .map(|x| x * 2)     // Returns Local<MapOp>
    .merge(other)       // Accepts only Local<T>
    .subscribe(...);

// Shared
rx::of(1)
    .into_shared()      // Returns Shared<Of>
    .map(|x| x * 2)     // Returns Shared<MapOp>, enforces F: Send
    .merge(other)       // Accepts only Shared<T>
    .subscribe(...);
```

## Pros & Cons

### Pros
*   **Explicit Boundaries**: Impossible to mix Local and Shared streams accidentally.
*   **Clear Error Messages**: Compiler errors occur at the method call site (e.g., "Local expects FnMut") rather than deep inside trait bounds.
*   **Unified API Surface**: Users see identical method names (`merge` vs `merge`) without suffixes, but types are strictly checked.
*   **Rust Idiomatic**: Uses the Type State / Newtype pattern which is very common in Rust libraries.

### Cons
*   **Boilerplate**: Requires macro maintenance to duplicate API surface.
*   **Abstraction Wall**: Hard to write generic functions `fn process<O: ???>(o: O)` that work for both Local and Shared, unless specific GAT traits are added (which brings back Scheme 3 complexities).

## Recommendation
This is the **recommended scheme** for its robustness and user-friendliness.
