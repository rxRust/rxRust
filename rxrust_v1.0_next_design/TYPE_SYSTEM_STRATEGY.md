# rxRust v1 Type System Strategy: Propagation, Storage, and Erasure

## 1. The Challenge: "Generic Explosion"

In rxRust v1, Observables are built using **Zero-Cost Abstractions**. Every operator creates a new, unique type wrapping the previous one.

**Example:**
```rust
// The type of `stream` is:
// LocalCtx<Map<Filter<Of<i32>, ...>, ...>, LocalScheduler>
let stream = Local::of(1)
    .filter(|v| v > 0)
    .map(|v| v * 2);
```

While highly efficient (everything mono-morphizes and inlines), this creates three specific friction points:
1.  **Storage**: You cannot define a `struct` field to hold `stream` without writing out that insane type.
2.  **Return Types**: You generally don't want to expose `Map<Filter<...>>` in your public API.
3.  **Heterogeneity**: You cannot put `Local::of(1)` and `Local::of(2).map(...)` in the same `Vec` because they are different types, even if they both emit `i32`.

This design document outlines the comprehensive strategy to handle these cases.

---

## 2. Strategy A: Type Erasure (Boxing)
**Best For:** Struct fields, Collections (`Vec`), Long-term storage, Public API boundaries where simplicity > raw perf.

### 2.1 The Components

We use a design where the **BoxedObserver (i.e., `Box<dyn DynObserver>`)** is used for type erasure. The boxing of subscriptions will be handled by the existing `IntoBoxedSubscription` trait.

1.  **`DynCoreObservable` Trait**: An object-safe trait for type-erased observables.
2.  **`BoxedCoreObservable` Struct**: A minimalist wrapper that holds the boxed observable.
3.  **Context-Specific Aliases**: We define separate type aliases for Local and Shared contexts, each with their corresponding BoxedObserver and BoxedSubscription types.

```rust
// 1. The Object-Safe Trait (defined per context variant)
pub trait DynCoreObservable<'a, Item, Err> {
  fn dyn_subscribe(
    self: Box<Self>,
    dyn_observer: BoxedObserver<'a, Item, Err>
  ) -> BoxedSubscription;
}

// 2. The Wrapper
pub struct BoxedCoreObservable<B>(pub(crate) B);
```

### 2.2 Integration with `Observable` Trait

We integrate `box_it` and `box_it_mut_ref` directly into the `Observable` trait.

**Observable Trait Definition (`src/v1/observable.rs`):**

```rust
pub trait Observable: Context {
  // ... existing methods ...

  /// Type-erases the observable into a Boxed Value Observable.
  fn box_it<'a>(self) -> Self::With<Self::BoxedCoreObservable<'a, Self::Item<'a>, Self::Err>>
  where
      Self::Inner: IntoBoxedCoreObservable<'a, Self::Item<'a>, Self::Err, Self::Scheduler>
  {
      self.transform(|inner| inner.into_boxed())
  }

  /// Type-erases the observable into a Boxed Mutable Reference Observable.
  fn box_it_mut_ref<'a>(self) -> Self::With<Self::BoxedCoreObservableMutRef<'a, Self::Item<'a>, Self::Err>>
  where
      Self::Inner: IntoBoxedCoreObservableMutRef<'a, Self::Item<'a>, Self::Err, Self::Scheduler>
  {
      self.transform(|inner| inner.into_boxed_mut_ref())
  }
}
```

### 2.3 Type Aliases for Ergonomics

We define specific type aliases for Local/Shared environments, covering both Value and Mutable Reference use cases. These are defined as associated types on the `Context` trait:

```rust
// In Context trait
type BoxedCoreObservable<'a, Item, Err>;
type BoxedCoreObservableMutRef<'a, Item: 'a, Err>;
```

#### Usage in prelude
```rust
// Type aliases for convenience (in rxrust::v1::prelude)

/// A type-erased Observable running in a Local context.
pub type LocalBoxedObservable<'a, Item, Err, S> = LocalCtx<
    BoxedCoreObservable<'a, Item, Err, S>, S
>;

/// A type-erased Observable running in a Shared context.
pub type SharedBoxedObservable<'a, Item, Err, S> = SharedCtx<
    BoxedCoreObservableSend<'a, Item, Err, S>, S
>;
```

### 2.4 Usage Scenarios

**Scenario: Component State**
```rust
struct ViewModel {
    // Stores ANY Observable that emits Strings
    input_stream: LocalBoxedObservable<'static, String>,
    // Stores ANY Observable that modifies Integers
    modifier_stream: LocalBoxedMutRefObservable<'static, i32>,
}

impl ViewModel {
    fn new() -> Self {
        let raw = Local::from_iter(vec!["a", "b"]).map(|s| s.to_uppercase());
        let mods = Local::mut_ref_subject();
            
        Self {
            input_stream: raw.box_it(),
            modifier_stream: mods.box_it_mut_ref(),
        }
    }
}
```

---

## 3. Strategy B: Opaque Types (`impl Trait`)
**Best For:** Function return values, Public API boundaries where you want to hide implementation but keep zero-cost performance.

Rust's `impl Trait` feature allows us to return a type without naming it, as long as the caller only relies on the trait bounds.

### 3.1 The Pattern

```rust
fn create_ticker() -> impl Observable<Item<'static> = i32, Err = ()> {
    Local::interval(Duration::from_secs(1))
        .map(|_| 1)
        .scan(0, |acc, v| acc + v)
}
```

### 3.2 Constraints & Solutions

**Constraint:** `impl Trait` in return types leaks "unnameable types".
*   **Solution:** If the caller needs to store it, *they* can call `.box_it()` on the result.

**Constraint:** Conditional returns.
*   **Solution:** Use `Strategy A (Boxing)` for this case.

---

## 4. Strategy C: Generic Propagation
**Best For:** Library authors, internal helper functions, high-performance loops.

This is the standard Rust approach. We accept generics and return generics.

### 4.1 Input Arguments
When a function accepts an Observable, it should always be generic to allow maximum flexibility.

```rust
// ✅ Good: Accepts any Observable emitting i32
fn subscribe_and_log<O>(stream: O)
where 
    O: Observable<Err=()>,
    for<'a> O::Item<'a>: Display
{
    stream.subscribe(|v| println!("{}", v));
}
```

---

## 5. Summary & Decision Matrix

When designing APIs or Data Structures in rxRust v1, use this decision matrix:

| Use Case | Recommended Strategy | Tool / Type | Pros | Cons |
| :--- | :--- | :--- | :--- | :--- |
| **Struct Fields** | **Type Erasure** | `LocalBoxedObservable` / `SharedBoxedObservable` | Clean, simple types. | Heap allocation, dynamic dispatch overhead. |
| **Return Types** | **Opaque Types** | `-> impl Observable` | Zero-cost, hides implementation. | Result type is unnameable (cannot be stored easily). |
| **Conditional Return** | **Type Erasure** | `-> LocalBoxedObservable` | Works with `if/else` branches returning different operators. | Allocation. |
| **Function Arguments** | **Generics** | `fn foo<O: Observable>(...)` | Flexible, allows user to pass anything. | Verbose signature. |
| **Operator Implementation**| **Generics** | `struct Map<S, F> ...` | Zero-cost, idiomatic Rust. | "Generic explosion". |

## 6. Implementation Plan for "Type Storage"

To implement this vision, we need to complete the following:

1.  **Context Enhancement**: Add `BoxedCoreObservable` / `BoxedCoreObservableMutRef` / `BoxedSubscription` types to `Context` trait.
2.  **Core Implementation**: Implement `src/v1/observable/boxed.rs` with `DynCoreObservable` trait and `BoxedCoreObservable` wrapper.
3.  **Observable Integration**: Add `box_it` and `box_it_mut_ref` methods to `Observable` trait.
4.  **Type Aliases**: Add convenient type aliases to `src/v1/prelude.rs`.