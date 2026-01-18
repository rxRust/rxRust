# Type Erasure

One common challenge when working with rxRust (and Rust iterators/futures in general) is the rapidly growing complexity of types.

## The Problem: Type Explosion

Every operator you apply wraps the previous Observable in a new struct.

```rust
use rxrust::prelude::*;

// Type: Local<Map<Filter<FromIter<...>, ...>, ...>>
let stream = Local::from_iter(0..10)
    .filter(|v| v % 2 == 0)
    .map(|v| v * 2);
```

While these types are usually inferred by the compiler, they become problematic when you need to:
1. Return an Observable from a function.
2. Store an Observable in a struct.
3. Conditionally return different Observable chains.
4. Store heterogeneous observables in a collection.

## The Solution: `box_it()` (Type Erasure)

When you need to store an Observable or return different types from different branches, use **Type Erasure** via the `.box_it()` operator. This boxes the observable, unifying the type at the cost of heap allocation and dynamic dispatch.

### Basic Usage

```rust
use rxrust::prelude::*;
use std::convert::Infallible;

fn create_dynamic_stream(condition: bool) -> LocalBoxedObservable<'static, i32, Infallible> {
    if condition {
        Local::of(1).box_it()
    } else {
        Local::from_iter(0..10).map(|v| v * 2).box_it()
    }
}

fn main() {
    let stream = create_dynamic_stream(true);
    stream.subscribe(|v| println!("{}", v));
}
```

### Storing in Collections

```rust
use rxrust::prelude::*;
use std::convert::Infallible;

// Different source types become the same boxed type
let boxed1: LocalBoxedObservable<'_, i32, Infallible> = Local::of(1).box_it();
let boxed2: LocalBoxedObservable<'_, i32, Infallible> = 
    Local::from_iter([2, 3]).map(|x| x * 2).box_it();

// Now they can be stored together
let observables = vec![boxed1, boxed2];

for obs in observables {
    obs.subscribe(|v| println!("{}", v));
}
```

**Pros:**
- Returns a unified concrete type.
- Can be stored in structs and collections easily.
- Works across conditional branches.

**Cons:**
- Heap allocation for the boxed observable.
- Dynamic dispatch overhead (usually negligible).

> **Note on Error Types**: When using `.subscribe(|v| ...)` with a simple closure, the observable's error type must be `Infallible`. This is because the closure-based subscription cannot handle errors. If you need a different error type, use `.subscribe_with()` with a full `Observer` implementation.

## Boxed Type Variants

rxRust provides multiple boxed observable types to cover different use cases:

### By Context (Local vs Shared)

| Context | Type | Created with |
|---------|------|--------------|
| Local (single-threaded) | `LocalBoxedObservable<'a, Item, Err>` | `Local::...box_it()` |
| Shared (multi-threaded) | `SharedBoxedObservable<'a, Item, Err>` | `Shared::...box_it()` |

### By Item Type (Value vs Mutable Reference)

| Item Type | Method | Result Type |
|-----------|--------|-------------|
| Owned values (`T`) | `.box_it()` | `LocalBoxedObservable` / `SharedBoxedObservable` |
| Mutable references (`&mut T`) | `.box_it_mut_ref()` | `LocalBoxedObservableMutRef` / `SharedBoxedObservableMutRef` |

### By Cloneability

| Cloneable? | Method | Result Type |
|------------|--------|-------------|
| No | `.box_it()` | `LocalBoxedObservable` |
| Yes | `.box_it_clone()` | `LocalBoxedObservableClone` |
| No (mut ref) | `.box_it_mut_ref()` | `LocalBoxedObservableMutRef` |
| Yes (mut ref) | `.box_it_mut_ref_clone()` | `LocalBoxedObservableMutRefClone` |

## Cloneable Boxed Observables

The regular `box_it()` result is **not** `Clone`, because a boxed trait object cannot be cloned unless the underlying observable pipeline is cloneable.

If you need a boxed observable that can be cloned (for example, to keep a template observable and create copies for multiple independent subscriptions), use `box_it_clone()`.

```rust
use rxrust::prelude::*;
use std::convert::Infallible;

fn create_cloneable_stream() -> LocalBoxedObservableClone<'static, i32, Infallible> {
    // Works when the underlying pipeline is `Clone`
    Local::of(42).box_it_clone()
}

fn main() {
    let boxed = create_cloneable_stream();
    let boxed2 = boxed.clone();

    boxed.subscribe(|v| {
        assert_eq!(v, 42);
    });

    boxed2.subscribe(|v| {
        assert_eq!(v, 42);
    });
}
```

## Mutable Reference Observables

For observables that emit mutable references (`Item = &'a mut T`), use the `_mut_ref` variants:

```rust
use std::convert::Infallible;
use rxrust::prelude::*;

// Create a subject that emits mutable references
let subject = Local::subject_mut_ref::<i32, Infallible>();

// Box it for type erasure
let boxed = subject.clone().box_it_mut_ref();

// Or if you need it to be cloneable:
let boxed_clone = subject.clone().box_it_mut_ref_clone();
```

## Shared Context Example

For multi-threaded scenarios, use the `Shared` context:

```rust
use rxrust::prelude::*;
use std::convert::Infallible;

fn create_shared_stream(condition: bool) -> SharedBoxedObservable<'static, i32, Infallible> {
    if condition {
        Shared::of(1).box_it()
    } else {
        Shared::from_iter(0..10).map(|v| v * 2).box_it()
    }
}

// The returned observable can be sent across threads
```

## Complete Type Reference

| Context | Cloneable | Item Type | Type Alias |
|---------|-----------|-----------|------------|
| Local | No | `T` | `LocalBoxedObservable<'a, Item, Err>` |
| Local | Yes | `T` | `LocalBoxedObservableClone<'a, Item, Err>` |
| Local | No | `&mut T` | `LocalBoxedObservableMutRef<'a, Item, Err>` |
| Local | Yes | `&mut T` | `LocalBoxedObservableMutRefClone<'a, Item, Err>` |
| Shared | No | `T` | `SharedBoxedObservable<'a, Item, Err>` |
| Shared | Yes | `T` | `SharedBoxedObservableClone<'a, Item, Err>` |
| Shared | No | `&mut T` | `SharedBoxedObservableMutRef<'a, Item, Err>` |
| Shared | Yes | `&mut T` | `SharedBoxedObservableMutRefClone<'a, Item, Err>` |

## Summary

| Scenario | Recommendation |
|----------|----------------|
| Return different types from branches | `box_it()` |
| Store Observable in struct fields | `box_it()` |
| API boundaries | `box_it()` (easier for consumers) |
| Collections of heterogeneous observables | `box_it()` |
| Need `Clone` on boxed observable | `box_it_clone()` |
| Mutable reference observables | `box_it_mut_ref()` / `box_it_mut_ref_clone()` |
| Multi-threaded / cross-thread usage | Use `Shared::...box_it()` |
