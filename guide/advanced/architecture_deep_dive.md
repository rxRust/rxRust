# Architecture Deep Dive

![rxRust Architecture Diagram](architecture_diagram.jpeg)

To truly leverage rxRust's power and extend it effectively, a deeper understanding of its core architecture is crucial. rxRust v1.0 is built around three fundamental concepts: the `Context` trait, the `CoreObservable` trait, and the `ObservableType` trait.

## Context as Environment

The `Context` trait is the central pillar of rxRust's unified architecture. It acts as a **stateful environment carrier** that propagates through your observable chain. Instead of operators needing to be aware of whether they are running in a single-threaded (`Local`) or multi-threaded (`Shared`) environment, they simply interact with the `Context`.

This trait is responsible for:

*   **Carrying the Scheduler**: Every `Context` instance holds a `Scheduler` (e.g., `LocalScheduler`, `TokioScheduler`). This allows time-based operators like `delay` or `interval` to automatically use the correct underlying timer mechanism without any explicit passing.
*   **Managing Ownership Strategy**: The `Context` defines how inner values are managed (`Rc<RefCell<T>>` for `LocalCtx`, `Arc<Mutex<T>>` for `SharedCtx`). Operators simply use `Context::RcMut<T>` or `Context::RcCell<T>` and the `Context` handles the concurrency details.
*   **Type Propagation**: Through its `With<T>` associated type, the `Context` ensures that the scheduler type is preserved throughout the operator chain, guaranteeing consistency and type safety.

### Key Methods of `Context`

Let's look at some critical methods from the `Context` trait (defined in `src/context.rs`):

```rust ignore

pub trait Context: Sized {
    type Inner;
    type Scheduler: Clone + Default;
    type RcMut<T>: From<T> + Clone + RcDerefMut<Target = T>;
    type RcCell<T: Copy>: SharedCell<T>;

    // Crucial for type propagation
    type With<T>: Context<Inner = T, Scheduler = Self::Scheduler, RcMut<T> = Self::RcMut<T>>;

    fn from_parts(inner: Self::Inner, scheduler: Self::Scheduler) -> Self;
    fn new(inner: Self::Inner) -> Self;
    fn lift<U>(inner: U) -> Self::With<U>;

    // Used by operators to wrap observers or transform the inner value
    fn transform<U, F>(self, f: F) -> Self::With<U> where F: FnOnce(Self::Inner) -> U;

    // Used at subscription boundaries to replace the operator logic with the observer
    fn swap<U>(self, inner: U) -> (Self::Inner, Self::With<U>);

    fn scheduler(&self) -> &Self::Scheduler;
    // ... other methods ...
}
```

As you can see, the `Context` trait abstracts away the specifics of the execution environment, allowing operator implementations to remain clean and environment-agnostic.

## Type Substitution and Propagation via `With<T>`

One of the most powerful and somewhat complex aspects of the `Context` trait is its Generic Associated Type (GAT): `Self::With<T>`. This GAT is the mechanism by which rxRust achieves **type propagation** and allows for **environment-agnostic operator chaining**.

When you apply an operator, it typically takes a `Context` (which wraps the previous operator's logic) and produces a *new* `Context` that wraps its *own* logic. The `With<T>` type ensures that the `Scheduler` type and other environment-specific types (`RcMut`, `RcCell`) are **preserved** through this transformation.

Consider an operator chain like this:

`Local::of(1).map(|v| v + 1).filter(|v| v % 2 == 0).subscribe(...)`

1.  `Local::of(1)` creates a `LocalCtx<Of<i32>, LocalScheduler>`. Here, `Of<i32>` is the `Inner` type, and `LocalScheduler` is the `Scheduler`.
2.  When `map` is applied, it takes the `LocalCtx<Of<i32>, LocalScheduler>` and transforms it. Internally, `map` uses `Context::transform` to wrap the downstream observer. The output of `map` is a new context, for example, `LocalCtx<Map<Of<i32>, F>, LocalScheduler>`. Notice how `LocalScheduler` is carried forward by `With<T>`.
3.  Similarly, `filter` takes the `LocalCtx<Map<Of<i32>, F>, LocalScheduler>` and produces `LocalCtx<Filter<Map<Of<i32>, F>, F2>, LocalScheduler>`.

This continuous propagation of the `Scheduler` (and `RcMut`/`RcCell`) through the `With<T>` GAT means that any operator in the chain, regardless of its position, can access the correct execution environment without explicit passing or runtime overhead. It's a compile-time dependency injection system.

## The `CoreObservable` Trait (The Logic Kernel)

The `CoreObservable<O>` trait represents the **pure, environment-agnostic logic** of an observable sequence. It acts as a **Publisher** that knows how to subscribe an observer `O`.

```rust ignore

pub trait CoreObservable<O>: ObservableType {
    type Unsub: Subscription;
    
    fn subscribe(self, observer: O) -> Self::Unsub;
}
```

Crucially, `O` here is typically a `Context<Inner = MyObserver, Scheduler = MyScheduler>`. This is where the `CoreObservable` interacts with the execution environment provided by the `Context`.

When implementing a custom operator, you will implement `CoreObservable<C>` (where `C` is a generic `Context`) for your operator struct. This implementation defines how your operator's logic connects to the source observable and how it passes events to the downstream observer wrapped within the `Context`.

## The `ObservableType` Trait (Type Safety)

The `ObservableType` trait is a simple, dedicated trait for defining the `Item` and `Err` types produced by an observable. Its purpose is to decouple type definition from the subscription logic, which simplifies generic bounds and improves type inference.

```rust
pub trait ObservableType {
    type Item<'a> where Self: 'a;
    type Err;
}
```

Every operator struct implements `ObservableType` to declare what kind of data it emits and what errors it might produce. This allows the user-facing `Observable` trait to provide clean type aliases for `Item` and `Err`, making the API more ergonomic.

By understanding these three core traits, you gain the insight needed to build powerful, custom extensions for rxRust that seamlessly integrate into its unified, zero-cost abstraction model.
