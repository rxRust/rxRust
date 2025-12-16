# Type Erasure

One common challenge when working with RxRust (and Rust iterators/futures in general) is the rapidly growing complexity of types.

## The Problem: Type Explosion

Every operator you apply wraps the previous Observable in a new struct.

```rust
use rxrust::prelude::*;

// Type: LocalObservable<Map<Filter<Range, ...>, ...>, ...>
let stream = Local::from_iter(0..10)
    .filter(|v| v % 2 == 0)
    .map(|v| v * 2);
```

While these types are usually inferred by the compiler, they become problematic when you need to:
1. Return an Observable from a function.
2. Store an Observable in a struct.
3. Conditionally return different Observable chains.

## Solution 1: `impl Observable` (Zero Cost)

If you are returning an Observable from a function, the preferred way is to use `impl Observable`. This keeps the concrete type hidden but maintains static dispatch (zero runtime overhead).

```rust
use rxrust::prelude::*;
use std::convert::Infallible;

fn create_stream() -> impl for<'a> Observable<Item<'a> = i32, Err = Infallible> {
    Local::from_iter(0..10)
        .filter(|v| v % 2 == 0)
        .map(|v| v * 2)
}
```

**Pros:**
- Zero runtime cost.
- No heap allocation.

**Cons:**
- Cannot be stored in a struct field (unless generic).
- All code paths must return the exact same concrete type.

## Solution 2: `box_it()` (Dynamic Dispatch)

When you need to store an Observable or return different types from different branches, you need **Type Erasure**. The `.box_it()` operator boxes the observable, unifying the type to `LocalBoxedObservable` (or `SharedBoxedObservable`).

```rust
use rxrust::prelude::*;

fn create_dynamic_stream(condition: bool) -> LocalBoxedObservable<'static, i32, ()> {
    if condition {
        Local::of(1).map_err(|_| ()).box_it()
    } else {
        Local::from_iter(0..10).map(|v| v * 2).map_err(|_| ()).box_it()
    }
}
```

**Pros:**
- Returns a unified concrete type (one of the 4 boxed variants, e.g., `LocalBoxedObservable`).
- Can be stored in structs easily.

### Cloneable boxed observables: `box_it_clone()` / `box_it_mut_ref_clone()`

The regular `box_it()` result is not `Clone`, because a boxed trait object
cannot be cloned unless the underlying observable pipeline is cloneable.

If you need a boxed observable that can be cloned (for example, to keep a
template observable in a struct and create copies for multiple independent
subscriptions), use `box_it_clone()`.

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

For observables that emit mutable references (`Item = &'a mut T`), use
`box_it_mut_ref_clone()`.

## Summary

| Scenario | Recommendation |
|----------|----------------|
| Function return type | `impl Observable` |
| Conditional branches | `box_it()` |
| Struct fields | `box_it()` or Generic parameters |
| API boundaries | `box_it()` (usually easier for consumers) |
| Need `Clone` on boxed observable | `box_it_clone()` / `box_it_mut_ref_clone()` |
