//! Observer trait and implementations
//!
//! The Observer trait defines the consumer of data in the reactive pattern.
//! It provides three methods: next (for values), error (for errors), and
//! complete (for stream completion).

use std::convert::Infallible;

use crate::context::{MutArc, MutRc, RcDeref, RcDerefMut};

// ============================================================================
// Observer Trait
// ============================================================================

/// Observer trait: The consumer of data in reactive programming
///
/// An Observer receives values, errors, and completion notifications from
/// an Observable.
pub trait Observer<Item, Err> {
  /// Receive the next value from the observable
  fn next(&mut self, value: Item);

  /// Handle an error from the observable
  ///
  /// This consumes the observer, as no more values can be emitted after an
  /// error
  fn error(self, err: Err);

  /// Handle completion of the observable
  ///
  /// This consumes the observer, as no more values can be emitted after
  /// completion
  fn complete(self);

  /// Checks if the observer is closed.
  ///
  /// This is primarily used by sources (like `from_iter`) to determine
  /// if they should stop emitting values early (e.g., due to a `Take`
  /// operator).
  ///
  /// Returns `true` if the observer is closed and will not accept more values.
  fn is_closed(&self) -> bool;
}

// ============================================================================
// Emitter Trait
// ============================================================================

/// A trait for emitting items to an Observer via mutable reference.
///
/// Unlike `Observer`, which consumes `self` for `error` and `complete` to
/// signal terminal states, `Emitter` takes `&mut self` for all methods. This
/// design choice serves two critical purposes:
///
/// 1. **Enabling Zero-Cost Type Erasure**: By allowing `&mut dyn Emitter`, it
///    facilitates dynamic dispatch (vtable lookup) without requiring heap
///    allocations (`Box<dyn Trait>`) for the observer itself. This is crucial
///    for performance-sensitive scenarios where allocating an `Observer` on the
///    heap for every `create` operation would be inefficient.
/// 2. **Preventing Leaky Abstractions**: In the context of the `create`
///    operator, the upstream `Observable` should not need to know the concrete
///    type of the downstream `Observer` (which could be a `MapObserver`,
///    `FilterObserver`, a `Subject`, or the final user-provided closure).
///    Passing a generic `&mut dyn Emitter` allows the `create` closure to
///    interact with any `Observer` that correctly implements the `Emitter`
///    facade, thereby maintaining Observable's generality and preventing the
///    concrete `Observer` type from "leaking" into the `Observable`'s type
///    signature.
///
/// This approach ensures a clean separation of concerns, where the `create`
/// function defines a generic emission strategy, decoupled from the specific
/// observer implementation down the chain.
///
/// # Future Possibilities: Higher-Ranked Types (HRT)
///
/// Currently, Rust only supports Higher-Ranked Trait Bounds (HRTB) for
/// lifetimes (e.g., `for<'a>`). If Rust eventually supports general
/// Higher-Ranked Types (Rank-2 Polymorphism) for types (e.g., `for<T>`), it
/// might be possible to achieve this same decoupling using pure static dispatch
/// without `dyn`. We could theoretically define a closure that accepts "any
/// type `O` that implements `Emitter`", allowing the compiler to monomorphize
/// the closure for each specific observer type at the call site, thus
/// eliminating the vtable overhead entirely.
///
/// ```rust,ignore
/// // Hypothetical Syntax for Rank-2 Polymorphism
/// fn create<F>(f: F) -> Create<F>
/// where
///     // The closure F is generic over the Observer type O!
///     F: for<O: Observer<Item, Err>> FnOnce(O) -> Subscription
/// { /* ... */ }
/// ```
///
/// Until then, `&mut dyn Emitter` remains the optimal solution.
pub trait Emitter<Item, Err> {
  fn next(&mut self, value: Item);
  fn error(&mut self, err: Err);
  fn complete(&mut self);
}

// ============================================================================
// DynObserver Trait - Object-safe Observer
// ============================================================================

/// Helper trait to enable object-safe Observers (Box<dyn Observer>)
///
/// Standard Observer trait is not object-safe because methods take `self` by
/// value or have generic parameters. DynObserver mirrors the interface but
/// adapts it for vtables.
pub trait DynObserver<Item, Err> {
  fn box_next(&mut self, value: Item);
  fn box_error(self: Box<Self>, err: Err);
  fn box_complete(self: Box<Self>);
  fn box_is_closed(&self) -> bool;
}

impl<T, Item, Err> DynObserver<Item, Err> for T
where
  T: Observer<Item, Err>,
{
  fn box_next(&mut self, value: Item) { self.next(value); }
  fn box_error(self: Box<Self>, err: Err) { self.error(err); }
  fn box_complete(self: Box<Self>) { self.complete(); }
  fn box_is_closed(&self) -> bool { self.is_closed() }
}

// ============================================================================
// Observer implementations for Box<dyn DynObserver>
// ============================================================================

/// Macro to implement Observer trait for boxed DynObserver types
/// This reduces code duplication by extracting common method implementations
macro_rules! impl_observer_for_box {
  // Internal: Common implementation for all Observer trait methods
  (@impl $ty:ty, generics = [$($generics:tt)*], item_type = $item:ty) => {
    #[allow(coherence_leak_check)]
    impl<$($generics)*> Observer<$item, Err> for $ty {
      #[inline]
      fn next(&mut self, value: $item) {
        (**self).box_next(value)
      }

      #[inline]
      fn error(self, err: Err) {
        self.box_error(err)
      }

      #[inline]
      fn complete(self) {
        self.box_complete()
      }

      #[inline]
      fn is_closed(&self) -> bool {
        (**self).box_is_closed()
      }
    }
  };

  // Public API: Regular observer (Observer<Item, Err>)
  (regular: $ty:ty, lifetime = $lf:lifetime) => {
    impl_observer_for_box!(@impl $ty, generics = [$lf, Item, Err], item_type = Item);
  };

  // Public API: Mutable reference observer (Observer<&mut T, Err>)
  (mut_ref: $ty:ty, lifetime = $lf:lifetime) => {
    impl_observer_for_box!(@impl $ty, generics = [$lf, T, Err], item_type = &mut T);
  };
}

// Regular observers (Item: Clone pattern)
impl_observer_for_box!(regular: Box<dyn DynObserver<Item, Err> + 'a>, lifetime = 'a);
impl_observer_for_box!(regular: Box<dyn DynObserver<Item, Err> + Send + 'a>, lifetime = 'a);
// HRTB mutable reference observers (&mut T pattern)
impl_observer_for_box!(mut_ref: Box<dyn for<'m> DynObserver<&'m mut T, Err> + 'a>, lifetime = 'a);
impl_observer_for_box!(
  mut_ref: Box<dyn for<'m> DynObserver<&'m mut T, Err> + Send + 'a>,
  lifetime = 'a
);

// ============================================================================
// IntoBoxedObserver Trait
// ============================================================================

/// Helper trait to convert observers into boxed trait objects
///
/// This trait enables type-safe conversion of concrete observer types into
/// the appropriate boxed observer type (with or without Send bound) based on
/// the context (Local vs Shared).
pub trait IntoBoxedObserver<O> {
  fn into_boxed(self) -> O;
}

// Blanket implementation for Local context (no Send requirement)
impl<'a, Item, Err, O> IntoBoxedObserver<Box<dyn DynObserver<Item, Err> + 'a>> for O
where
  O: Observer<Item, Err> + 'a,
{
  fn into_boxed(self) -> Box<dyn DynObserver<Item, Err> + 'a> { Box::new(self) }
}

// Blanket implementation for Shared context (Send requirement)
impl<'a, Item, Err, O> IntoBoxedObserver<Box<dyn DynObserver<Item, Err> + Send + 'a>> for O
where
  O: Observer<Item, Err> + Send + 'a,
{
  fn into_boxed(self) -> Box<dyn DynObserver<Item, Err> + Send + 'a> { Box::new(self) }
}

// Blanket implementation for Local context (MutRefObserver)
#[allow(coherence_leak_check)]
impl<'a, T, Err, O> IntoBoxedObserver<Box<dyn for<'m> DynObserver<&'m mut T, Err> + 'a>> for O
where
  O: for<'m> Observer<&'m mut T, Err> + 'a,
{
  fn into_boxed(self) -> Box<dyn for<'m> DynObserver<&'m mut T, Err> + 'a> { Box::new(self) }
}

// Blanket implementation for Shared context (MutRefObserver)
#[allow(coherence_leak_check)]
impl<'a, T, Err, O> IntoBoxedObserver<Box<dyn for<'m> DynObserver<&'m mut T, Err> + Send + 'a>>
  for O
where
  O: for<'m> Observer<&'m mut T, Err> + Send + 'a,
{
  fn into_boxed(self) -> Box<dyn for<'m> DynObserver<&'m mut T, Err> + Send + 'a> { Box::new(self) }
}

// ============================================================================
// Type Aliases for Boxed Observer Types
// ============================================================================

/// Boxed value observer (single-threaded, no Send bound)
pub type BoxedObserver<'a, Item, Err> = Box<dyn DynObserver<Item, Err> + 'a>;

/// Boxed value observer with Send bound (multi-threaded)
pub type BoxedObserverSend<'a, Item, Err> = Box<dyn DynObserver<Item, Err> + Send + 'a>;

/// Boxed mutable reference observer using HRTB (single-threaded, no Send bound)
pub type BoxedObserverMutRef<'a, Item, Err> = Box<dyn for<'m> DynObserver<&'m mut Item, Err> + 'a>;

/// Boxed mutable reference observer using HRTB with Send bound (multi-threaded)
pub type BoxedObserverMutRefSend<'a, Item, Err> =
  Box<dyn for<'m> DynObserver<&'m mut Item, Err> + Send + 'a>;

// ============================================================================
// FnMutObserver - Closure adapter
// ============================================================================

/// Blanket implementation of Observer for closures
///
/// This enables ergonomic subscription syntax: `observable.subscribe(|v|
/// println!("{}", v))` The closure becomes the `next` handler, while `error`
/// and `complete` are ignored by default.
#[derive(Clone)]
pub struct FnMutObserver<F>(pub F);

impl<F, Item> Observer<Item, Infallible> for FnMutObserver<F>
where
  F: FnMut(Item),
{
  #[inline]
  fn next(&mut self, v: Item) { (self.0)(v); }

  #[inline]
  fn error(self, _err: Infallible) {
    // Default: ignore errors (unreachable for Infallible)
  }

  #[inline]
  fn complete(self) {
    // Default: ignore completion
  }

  #[inline]
  fn is_closed(&self) -> bool { false }
}

// ============================================================================
// Observer implementations for Option and reference-counted Option wrappers
// ============================================================================

/// Option observer - None ignores all events, Some delegates to inner
impl<O, Item, Err> Observer<Item, Err> for Option<O>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) {
    if let Some(inner) = self {
      inner.next(value);
    }
  }

  fn error(self, err: Err) {
    if let Some(inner) = self {
      inner.error(err);
    }
  }

  fn complete(self) {
    if let Some(inner) = self {
      inner.complete();
    }
  }

  fn is_closed(&self) -> bool { self.as_ref().is_none_or(Observer::is_closed) }
}

/// MutRc<Option<O>> - shared ownership observer for local context
/// Uses take() for terminal operations to consume the inner observer
impl<O, Item, Err> Observer<Item, Err> for MutRc<Option<O>>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) { self.rc_deref_mut().next(value); }

  fn error(self, err: Err) {
    if let Some(inner) = self.rc_deref_mut().take() {
      inner.error(err);
    }
  }

  fn complete(self) {
    if let Some(inner) = self.rc_deref_mut().take() {
      inner.complete();
    }
  }

  fn is_closed(&self) -> bool { self.rc_deref().is_none() }
}

/// MutArc<Option<O>> - shared ownership observer for shared context
/// Uses take() for terminal operations to consume the inner observer
impl<O, Item, Err> Observer<Item, Err> for MutArc<Option<O>>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) { self.rc_deref_mut().next(value); }

  fn error(self, err: Err) {
    if let Some(inner) = self.rc_deref_mut().take() {
      inner.error(err);
    }
  }

  fn complete(self) {
    if let Some(inner) = self.rc_deref_mut().take() {
      inner.complete();
    }
  }

  fn is_closed(&self) -> bool { self.rc_deref().is_none() }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
  use super::*;

  struct TestObserver {
    values: Vec<i32>,
  }

  impl Observer<i32, ()> for TestObserver {
    fn next(&mut self, value: i32) { self.values.push(value); }

    fn error(self, _: ()) {}

    fn complete(self) {}

    fn is_closed(&self) -> bool { false }
  }

  #[rxrust_macro::test]
  fn test_observer_trait() {
    let mut obs = TestObserver { values: vec![] };
    obs.next(1);
    obs.next(2);
    assert_eq!(obs.values, vec![1, 2]);
    assert!(!obs.is_closed());
  }

  #[rxrust_macro::test]
  fn test_closure_as_observer() {
    let mut count = 0;
    let mut closure_obs = FnMutObserver(|v: i32| {
      count += v;
    });

    closure_obs.next(10);
    closure_obs.next(20);
    assert_eq!(count, 30);
  }
}
