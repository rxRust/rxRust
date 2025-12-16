# Refactoring Scheme 1: Policy Associated Type

## Overview
This scheme maintains a single `Observable` trait but introduces a `Policy` associated type to distinguish between single-threaded (`Local`) and multi-threaded (`Shared`) contexts. Most operators simply pass through this policy, while concurrency-sensitive operators (like `merge` or `subject`) utilize it to adapt their internal implementation.

## Core Trait Definition

```rust
pub trait SharedPolicy {
    // Defines the reference counting strategy (Rc vs Arc)
    type Rc<T>: RcDeref<Target = T> + RcDerefMut<Target = T> + From<T>;
}

pub struct Local;
impl SharedPolicy for Local { type Rc<T> = MutRc<T>; }

pub struct Shared;
impl SharedPolicy for Shared { type Rc<T> = MutArc<T>; }

pub trait Observable<Item, Err>: Sized {
    // 1. Introduce Policy Marker
    type Policy: SharedPolicy;

    type Unsub: Subscription;

    fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
    where O: Observer<Item, Err>;

    // --- Operators (Standard) ---

    // Operators like 'map' are agnostic to the policy.
    // They simply inherit the policy from the source.
    fn map<B, F>(self, f: F) -> MapOp<Self, F, Item>
    where
        F: FnMut(Item) -> B,
    {
        MapOp::new(self, f)
    }

    // --- Operators (Concurrency Sensitive) ---

    // 'merge' adapts its return type based on the Policy.
    // The 'MergeOp' struct handles the difference internally via the Policy generic.
    fn merge<S>(self, other: S) -> MergeOp<Self, S, Self::Policy>
    where
        S: Observable<Item, Err, Policy = Self::Policy>,
    {
        MergeOp::new(self, other)
    }
    
    // Switch to Shared Context
    fn into_shared(self) -> SharedWrapper<Self> {
        SharedWrapper(self)
    }
}
```

## Implementation Details

### 1. Operator Structs
Most operator structs don't change much, but they need to carry the `Policy` in their `impl Observable`.

**MapOp:**
```rust
// Struct remains the same
pub struct MapOp<S, F, Item> { source: S, ... }

// Impl inherits Policy
impl<S, F, ...> Observable for MapOp<S, F, Item> 
where S: Observable 
{
    type Policy = S::Policy; // Pass-through
    // ...
}
```

**MergeOp:**
```rust
// Struct already supports Policy generic
pub struct MergeOp<S1, S2, Policy> { ... }

impl<S1, S2, Policy> Observable for MergeOp<S1, S2, Policy> 
where Policy: SharedPolicy 
{
    type Policy = Policy;
    // ...
}
```

### 2. Usage Example

```rust
// Local (Default)
observable::of(1)         // Policy = Local
    .map(|x| x * 2)       // Policy = Local
    .merge(other)         // Returns MergeOp<..., Local>
    .subscribe(...);

// Shared
observable::of(1)
    .into_shared()        // Returns Wrapper with Policy = Shared
    .map(|x| x * 2)       // Policy = Shared
    .merge(other)         // Returns MergeOp<..., Shared> (Thread-safe)
    .subscribe(...);
```

## Pros & Cons

### Pros
*   **Single Trait**: Keeps the system unified under one `Observable` trait.
*   **Zero-Cost**: No runtime overhead; everything is resolved at compile time.
*   **Internal Consistency**: Leveraging the existing `SharedPolicy` abstraction in `rxRust` fits the codebase well.

### Cons
*   **Implicit Magic**: Users might not understand why `.merge()` behaves differently (requiring `Send` or not) based on an invisible associated type.
*   **Generic Pollution**: Every implementor of `Observable` must explicitly define `type Policy = ...`.
*   **API Pollution**: The `into_shared()` wrapper needs to be carefully implemented to proxy all methods correctly if it's a struct, or rely on the trait implementation.

## Feasibility
Verified via prototype. The compiler correctly propagates the associated type, allowing `merge` to polymorphically return the correct struct variant.
