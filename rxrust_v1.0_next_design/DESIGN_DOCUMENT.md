# rxRust Stable 1.0 Architecture Reference

## 1. Vision & Goals

The primary objective of the rxRust 1.0 refactoring is to **unify** the disparate single-threaded (`Local`) and multi-threaded (`Shared`) implementations into a single, cohesive codebase.

### Legacy State (Pre-1.0)
*   **Duplication**: Operators are implemented twice or via complex macros.
*   **Rigidity**: Adding a new operator requires defining multiple structs and trait implementations.
*   **Complexity**: `SharedPolicy` is mixed into operator structs, causing generic pollution.

### Target State (1.0 Stable)
*   **Zero Duplication**: One "Logic Struct" (e.g., `Map`) serves both `Local` and `Shared` modes.
*   **Unified Kernel**: The logic is driven by a single `CoreObservable<O>` trait (the **Publisher**).
*   **Type Safety**: The user-facing `Observable` trait strictly exposes `Item` and `Err` types via a dedicated `ObservableType` trait, ensuring clean and stable APIs.
*   **Stateful Context**: The `Context` carries the execution environment (Scheduler), enabling Dependency Injection for time-based operators.
*   **Flexible Scheduler Configuration**: Users can customize schedulers with zero API changes via type shadowing.
*   **Ergonomic Subscription**: Direct closure support for `next` values, with powerful operators for error and completion handling.

## 1.1 Design Principles

To ensure consistency and safety, the architecture strictly adheres to two core principles:

1.  **Unified Context Authority**: All concurrency variance (Local vs Shared) must be strictly managed by the `Context` trait. Logic structs (e.g., `Map`) must remain agnostic to the threading model, relying solely on `Context::Rc` and `Context::Scheduler`.
2.  **Contextual Propagation**: The `Context` travels with the `Observer`. During subscription, the `Observer` is "packed" into the `Context` (via `replace_inner`). This ensures that the execution environment (Scheduler) is automatically passed down the pipeline, allowing operators to access the correct scheduler at any point.

## 2. Core Architecture

The new architecture relies on three pillars: **Observer**, **Context**, and the **CoreObservable** logic.

### 2.1 The `Observer` Trait

The `Observer` serves as the consumer of data.

```rust
pub trait Observer<Item, Err> {
    fn next(&mut self, value: Item);
    fn error(self, err: Err);
    fn complete(self);

    /// Checks if the observer is closed.
    /// Defaults to `false`.
    fn is_closed(&self) -> bool { false }
}
```

To support ergonomic subscriptions (`obs.subscribe(|v| ...)`), we implement `Observer` directly for closures:

```rust
// Allows `subscribe(|v| println!("{}", v))`
impl<F, Item, Err> Observer<Item, Err> for F
where F: FnMut(Item)
{
    fn next(&mut self, v: Item) { (self)(v); }
    fn error(self, _: Err) {} // Default: Ignore error
    fn complete(self) {}      // Default: Ignore complete
    // is_closed() defaults to false
}
```

### 2.2 The `Context` Trait (Stateful Environment)

The `Context` trait defines the **execution environment** (Local vs Shared) and provides **scheduling capabilities**. It is **Stateful**, carrying a Scheduler.

```rust
pub trait Context: Sized {
    type Inner;
    
    // The Scheduler associated with this context
    type Scheduler: Clone;

    // The concurrency strategy for this context family (Rc vs Arc)
    type Rc<T>: From<T> + Clone;

    // A GAT Factory: "Type Substitution"
    // Crucially, 'With' preserves the Scheduler type
    type With<T>: Context<Inner = T, Scheduler = Self::Scheduler>;

    // Access the environment scheduler
    fn scheduler(&self) -> &Self::Scheduler;

    // 1. Transform Inner (Functor map) - Used inside Operators to wrap Observers
    fn map_inner<U, F>(self, f: F) -> Self::With<U>
    where 
        F: FnOnce(Self::Inner) -> U;

    // 2. Replace Inner - Used at Subscribe boundary to swap Logic with Observer
    fn replace_inner<U>(self, inner: U) -> (Self::Inner, Self::With<U>);

/// Create a new Context with the given inner value and default scheduler
fn create<U>(inner: U) -> Self::With<U>
where
  Self::Scheduler: Default;
}
```

### 2.3 `CoreObservable<O>` (The Logic Kernel)

This trait represents the **pure logic** of an observable sequence. It acts as a **Publisher**. 

```rust
pub trait CoreObservable<O>: ObservableType {
    type Unsub: Subscription;
    
    fn subscribe(self, observer: O) -> Self::Unsub;
}
```

### 2.4 The `Observable` API & The `ObservableType` Trait

The user-facing `Observable` trait acts as a facade. We separate type definition from subscription logic to ensure types are consistent across all execution environments.

#### 1. The Source of Truth: `ObservableType`
This trait purely exposes the types. Operators must implement this explicitly.

```rust
pub trait ObservableType {
    type Item<'a> where Self: 'a;
    type Err;
}
```

#### 2. The User-Facing `Observable` Trait
The `Observable` trait now simply requires its inner logic to implement `ObservableType`.

```rust
pub trait Observable: Context 
where
    Self::Inner: ObservableType
{
    // Clean inheritance from Inner
    type Item<'a> = <Self::Inner as ObservableType>::Item<'a> where Self: 'a;
    type Err = <Self::Inner as ObservableType>::Err;

    // Standard Subscribe
    fn subscribe<F>(self, f: F) -> <Self::Inner as CoreObservable<Self::With<F>>>::Unsub
    where
        F: for<'a> FnMut(Self::Item<'a>),
        Self::Inner: CoreObservable<Self::With<F>>
    {
        // ...
    }
}
```

### 2.5 Scheduler & Unified TaskHandle (Zero-Cost Abstraction)

We adopt a **Schedulable Trait Pattern** that bridges user types with Scheduler implementations, enabling both state-machine tasks and native Futures to be scheduled uniformly.

#### Core Traits

**`Schedulable<Sch>` - The Bridge Trait**

This trait converts a type into a Future with the help of a Scheduler. It decouples Task from specific Scheduler implementations.

```rust
/// Types that can be scheduled by a specific Scheduler
pub trait Schedulable<Sch> {
    type Future: Future<Output = ()>;
    fn into_future(self, scheduler: &Sch) -> Self::Future;
}

/// Blanket implementation: Any Future<Output = ()> is directly schedulable
/// No Scheduler assistance needed - just returns itself
impl<F, Sch> Schedulable<Sch> for F
where
    F: Future<Output = ()>,
{
    type Future = Self;
    fn into_future(self, _scheduler: &Sch) -> Self::Future { self }
}
```

**`Scheduler<S>` - The Scheduler Trait**

```rust
pub trait Scheduler<S>: Clone
where
    S: Schedulable<Self>,
{
    fn schedule(&self, source: S, delay: Option<Duration>) -> TaskHandle;
}
```

#### Task (Pure State Machine)

`Task<S>` is a pure state machine struct. It does **not** implement `Future` directly - the conversion is handled by `Schedulable` implementations provided by Scheduler implementors.

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskState {
    /// The task has finished.
    Finished,
    /// Run the task again immediately (cooperative yield).
    Yield,
    /// Run the task again after the specified duration.
    Sleeping(Duration),
}

pub struct Task<S> {
    pub state: S,
    pub handler: fn(&mut S) -> TaskState,
}

impl<S> Task<S> {
    pub fn new(state: S, handler: fn(&mut S) -> TaskState) -> Self {
        Self { state, handler }
    }
    
    pub fn step(&mut self) -> TaskState {
        (self.handler)(&mut self.state)
    }
}
```

#### Default Scheduler Implementations (feature = "scheduler")

When the `scheduler` feature is enabled, rxRust provides default implementations using the `SleepProvider` trait. This trait bridges the gap between the abstract `Task` state machine and platform-specific timers (Tokio or WASM).

```rust
#[cfg(feature = "scheduler")]
mod default_schedulers {
    // 1. Implement SleepProvider
    // This allows Task<S> to automatically implement Schedulable<LocalScheduler>
    impl SleepProvider for LocalScheduler {
        type SleepFuture = tokio::time::Sleep;
        
        fn sleep(&self, duration: Duration) -> Self::SleepFuture { 
            tokio::time::sleep(duration) 
        }
    }
    
    // 2. Scheduler implementation
    // No manual Schedulable impl for Task needed!
    impl<S> Scheduler<S> for LocalScheduler
    where
        S: Schedulable<Self> + 'static,
    {
        fn schedule(&self, source: S, delay: Option<Duration>) -> TaskHandle {
            let future = source.into_future(self);
            let wrapped = async move {
                if let Some(d) = delay {
                    self.sleep(d).await;
                }
                future.await;
            };
            let (remote, handle) = remote_handle(wrapped);
            spawn_local(remote);
            handle
        }
    }
}
```

#### Custom Scheduler Implementation

Users can disable the `scheduler` feature and provide their own implementations. The recommended approach is to implement the `SleepProvider` trait, which allows rxRust's internal `Task<S>` to automatically bridge to your scheduler.

```rust
#[derive(Clone, Copy, Default)]
pub struct MyScheduler;

// 1. Implement SleepProvider to handle time (delays/yields)
impl SleepProvider for MyScheduler {
  type SleepFuture = std::future::Ready<()>; // Or your runtime's timer future

  fn sleep(&self, _duration: Duration) -> Self::SleepFuture { 
      // Return a future that completes after duration
      std::future::ready(()) 
  }
}

// 2. Implement Scheduler for ANY Schedulable item (Tasks or Futures)
// Note: Task<S> automatically implements Schedulable<MyScheduler> 
// because MyScheduler implements SleepProvider.
impl<S> Scheduler<S> for MyScheduler
where
  S: Schedulable<MyScheduler> + 'static,
{
  fn schedule(&self, source: S, delay: Option<Duration>) -> TaskHandle {
    let future = source.into_future(self);
    // Custom scheduling logic (e.g., spawn the future)
    // spawn(future);
    TaskHandle::finished()
  }
}
```

#### The Unified `TaskHandle`

TaskHandle serves as both `Subscription` and `Future`:

```rust
#[derive(Clone)]
pub struct TaskHandle {
    /* Internal state for cancellation and completion signaling */
}

impl Subscription for TaskHandle { ... }
impl Future for TaskHandle { ... }
```

#### Type Relationship Diagram

```
┌─────────────────────────────────────────────────────────┐
│               Schedulable<Sch> trait                    │
│  ┌────────────────────┐    ┌─────────────────────────┐  │
│  │ Future<Output=()>  │    │ Task<S>                 │  │
│  │ (blanket impl)     │    │ (impl via SleepProvider)│  │
│  └────────────────────┘    └─────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│  Scheduler<S: Schedulable<Self>>                        │
│  schedule(source: S) -> TaskHandle                      │
│    └─> internally: source.into_future(self).await       │
└─────────────────────────────────────────────────────────┘
```

**Key Advantages**:
1. **No Generic Pollution**: `Task<S>` has no Scheduler generic
2. **No Circular Constraints**: `Schedulable` is a one-way bridge
3. **Native Future Support**: Blanket impl for all `Future<Output = ()>`
4. **Full Customization**: Scheduler implementors control sleep/yield logic
5. **Send Bounds at impl Level**: `LocalScheduler` vs `SharedScheduler` enforce Send requirements

**Async Integration: `from_future` and `from_stream`**

With the blanket `Schedulable` impl for Futures, async operators work naturally:

```rust
// from_future - Future is directly Schedulable
Local::from_future(async { fetch_data().await })
    .subscribe(|data| println!("{:?}", data));

// from_stream - wrap as Future, then schedule
Local::from_stream(stream::iter(vec![1, 2, 3]))
    .subscribe(|v| println!("{}", v));
```

### 2.6 Observable Factory Pattern (The Final Solution)

**The Challenge**: Users want to customize schedulers without:
- API duplication (`of_with_scheduler`, etc.)
- Heap allocations (`Box<dyn FnOnce()>`)
- Boilerplate in business logic

**The Solution**: Unified Factory Trait + Marker Type Shadowing

#### Design

```rust
/// Unified factory trait for creating Observables
/// Extends Context to provide factory methods
pub trait ObservableFactory: Context {
    
    fn of<T>(v: T) -> Self::With<Of<T>> 
    where 
        Self::Scheduler: Default 
    {
        Self::new(Of(v))
    }
    
    // Other factory methods: from_iter, timer, interval, etc.
}

// Blanket implementation for all Contexts
impl<C: Context> ObservableFactory for C {}
```

#### Default Implementation (rxRust provides)

```rust
// In rxrust::prelude
pub struct Local;  // Zero-sized marker type

// Local must implement Context (runtime methods panic)
impl Context for Local {
    type Inner = ();
    type Scheduler = LocalScheduler;
    type Rc<T> = MutRc<T>;
    type With<T> = LocalContext<T, LocalScheduler>;
    // ... impl methods ...
    fn of<T: Clone>(v: T) -> Self::With<Of<T>> {
        rxrust::context::Local {
            inner: Of(v),
            scheduler: Self::Scheduler::default(),
        }
    }
}
```

#### User Customization (Type Shadowing)

```rust
// In user's prelude (e.g., my_app::rx)
pub struct Local;  // Shadow rxRust's Local

impl Context for Local {
    type Scheduler = MyTokioScheduler;
    type With<T> = rxrust::context::Local<T, MyTokioScheduler>;
    // ...
}

impl rxrust::factory::ObservableFactory for Local {
    fn of<T: Clone>(v: T) -> Self::With<Of<T>> {
        rxrust::context::Local {
            inner: Of(v),
            scheduler: Default::default(),
        }
    }
}
```

#### Usage

```rust
// Default scenario
use rxrust::prelude::*;
Local::of(1).delay(...).subscribe(...);

// Custom scenario
use my_app::rx::*;  // Uses shadowed Local
Local::of(1).delay(...).subscribe(...);  // ✅ IDENTICAL API!
```

**Advantages**:
- ✅ **Zero API changes**: `Local::of(...)` works everywhere
- ✅ **Zero heap allocations**: Completely static dispatch
- ✅ **Type safe**: Enforced by trait bounds
- ✅ **Centralized configuration**: Define once in prelude

## 3. Subscription & Error Handling Strategy

We prioritize a clean, composable API over overloaded arguments.

1.  **`subscribe`**: Only accepts a **closure** `FnMut(Item)`.
    *   **Focus**: This optimizes for the 90% use case and significantly improves type inference for `Item`.
    *   **Advanced**: For full `Observer` implementations (like Subjects or custom structs), use `subscribe_with(observer)`.
    *   **NO Tuple Support**: We explicitly reject `subscribe((next, error))` to keep the API simple.

2.  **Error/Complete Handling**: Handled via **Operators**.
    *   `.on_error(|e| ...)` / `.tap_error(...)`: Intercept errors for side effects.
    *   `.on_complete(|| ...)`: Intercept completion for side effects.
    *   This ensures "Everything is a Stream" consistency and better composability.

## 4. Implementation Guidelines for Operators

When implementing an operator (e.g., `Map`), we strictly follow the **Context Unpacking Pattern** and **Explicit Type Separation**. 

**Crucially, `Local` and `Shared` do NOT implement `Observer`.** The operator must unpack the context to access the observer.

### Pattern

1.  **Struct**: Pure logic, generic.
    ```rust
    pub struct Map<S, F> { source: S, pub func: F }
    ```

2.  **Implement `ObservableType`**:
    Explicitly define types, independent of the subscription context.
    
    ```rust
    impl<S, F, Out> ObservableType for Map<S, F>
    where
        S: ObservableType,
        F: FnMut(S::Item<'_>) -> Out,
    {
        type Item<'a> = Out;
        type Err = S::Err;
    }
    ```

3.  **Observer Wrapper**:
    ```rust
    pub struct MapObserver<O, F> { observer: O, func: F }
    impl<O, F, In, Out, Err> Observer<In, Err> for MapObserver<O, F> ...
    ```

4.  **Implement `CoreObservable<C>`**:
    Implement the subscription logic.
    
    ```rust
    impl<S, F, C, Out> CoreObservable<C> for Map<S, F>
    where
        C: Context,
        C::Inner: Observer<Out, S::Err>,
        S: CoreObservable<C::With<MapObserver<C::Inner, F>>>,
        F: for<'a> FnMut(S::Item<'a>) -> Out,
    {
        type Unsub = S::Unsub;
        // ... subscribe implementation ...
    }
    ```

## 5. Migration Strategy

### Phase 1: Foundation
1.  **`src/context.rs`**: Define `Context`, `Local`, `Shared` with `map_inner` / `replace_inner`.
2.  **`src/observable.rs`**: Define `CoreObservable`, `Observable`, `ObservableType` and `DiscardingObserver` (hidden).
3.  **`src/observer.rs` / `src/subscription.rs`**: Standard definitions.
4.  **`src/factory.rs`**: Define unified `ObservableFactory` trait.

### Phase 2: Core Operators
Port key operators to `src/ops/`:
*   `Of`
*   `Map`
*   `Merge`
*   `OnError` / `OnComplete` (Crucial for the new subscription model)
*   `Delay` (Demonstrating Scheduler usage)
## 4. Subject Design

### 4.1 Overview

Subjects are dual-natured types that act as both **Observable** and **Observer**. The 1.0 design introduces a unified `Subject<O>` architecture:

```rust
pub struct Subject<O> {
    observers: O,  // O = MutRc<Subscribers> or MutArc<Subscribers>
}
```

**Key Innovations**:
1. **Subject<O> Pattern**: Storage container `O` is provided by Factory, Subject doesn't perceive Local/Shared
2. **Dual Observer Implementation**: Supports both `Clone` values and `&mut` references through reborrowing
3. **SmallVec + ID**: Zero heap allocation for ≤2 subscribers
4. **Chamber Removal**: Simplified implementation, explicit circular reference constraints

### 4.2 Core Components

#### Subject<O> - Multicast

```rust
// Quick creation
let subject = Subject::local::<i32, ()>();
let subject = Subject::shared::<String, Error>();

// Value broadcasting - Clone to all subscribers
subject.clone().subscribe(|v| println!("{}", v));

// &mut broadcasting - Sequential modification via reborrowing
let mut_subject = Subject::local::<&mut i32, ()>();
mut_subject.clone().subscribe(|v| *v += 1);
mut_subject.clone().subscribe(|v| *v *= 2);
let mut value = 0;
mut_subject.next(&mut value);
assert_eq!(value, 2);  // (0 + 1) * 2
```

#### BehaviorSubject<O, V> - Stateful Subject

```rust
let behavior = BehaviorSubject::local::<i32, ()>(42);
behavior.clone().subscribe(|v| println!("{}", v));  // Immediately receives 42
behavior.next(100);
assert_eq!(behavior.peek(), 100);
behavior.next_by(|v| v + 1);  // Update based on current value
```

### 4.3 Design Constraints

**Circular Reference Prevention**: Synchronous nested subscriptions are explicitly **not supported** to prevent memory leaks.

```rust
// ❌ Not supported - circular reference
subject.clone().subscribe(move |v| {
    subject.clone().subscribe(|_| { /* ... */ });
});

// ✅ Alternative - delay the outer subscription
subject.clone()
    .delay(Duration::ZERO)
    .subscribe(move |v| {
        subject.clone().subscribe(|_| { /* ... */ });
    });
```

**For detailed design, see [SUBJECT_DESIGN.md](SUBJECT_DESIGN.md)**

---

## 5. Implementation Roadmap
### Phase 3: Prelude
1.  Define default marker types in `src/prelude.rs`
2.  Implement factory traits for default schedulers
3.  Document the customization pattern in examples


### Phase 4: Subject Implementation
*   Implement `Subject<O>` with dual Observer support (Clone + &mut)
*   Implement `BehaviorSubject<O, V>` with current value storage
*   Remove legacy Chamber mechanism and 3 MutRef variants
*   Add quick creation methods (::local() / ::shared())
*   See [SUBJECT_DESIGN.md](SUBJECT_DESIGN.md) and [subject_poc_verification.md](subject_poc_verification.md)

### Phase 5: Cleanup

*   Remove legacy macros.
*   Update tests.
*   Migration guide for pre-1.0 users.

## 6. Appendix: Rejected Designs (Lessons Learned)

The following approaches were explored and rejected during the design process:

### 6.1 Scheme 1: Policy as Generic Parameter
*   **Idea**: `Observable<Item, Err, Policy>`.
*   **Outcome**: **Rejected**.
*   **Reason**: Severe generic pollution. Every operator required the `Policy` generic, making signatures verbose and hard to read. It also complicated type inference for `Item` and `Err`.

### 6.2 Scheme 2: The Wrapper Approach
*   **Idea**: `Local<Observable>` where `Local` wraps the logic.
*   **Outcome**: **Rejected**.
*   **Reason**: Type explosion. Operators returning `Local<Map<Local<Filter<...>>>>` created deeply nested types that were inefficient to compile and difficult to debug.

### 6.3 Scheme 5: Policy Context Trait
*   **Idea**: A `Context` trait similar to the final design but without the "Factory" pattern.
*   **Outcome**: **Refined**.
*   **Reason**: It laid the groundwork for the final design but lacked a clean way to instantiate observables with specific schedulers without extensive API duplication (`of_local`, `of_shared`).

### 6.4 Box<dyn Subscription> for Schedulers
*   **Idea**: `Scheduler::schedule` returning `Box<dyn Subscription>`.
*   **Outcome**: **Rejected**.
*   **Reason**: Unnecessary heap allocation for `LocalScheduler` (which completes synchronously). Replaced by `TaskHandle`, which allows zero-cost abstraction for local tasks while supporting async futures for shared tasks.