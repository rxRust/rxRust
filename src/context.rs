//! Context trait and Local/Shared implementations
//!
//! The Context trait provides the execution environment (Local vs Shared)
//! and carries the Scheduler for dependency injection.

// Re-export Rc types from the rc module
pub use crate::rc::*;
#[cfg(feature = "scheduler")]
use crate::scheduler::{LocalScheduler, SharedScheduler};
use crate::{
  observer::{
    BoxedObserver, BoxedObserverMutRef, BoxedObserverMutRefSend, BoxedObserverSend, Observer,
  },
  subscription::{BoxedSubscription, BoxedSubscriptionSend, Subscription},
};

// ==================== Scope Trait ====================

/// Scope defines environment capabilities (Local vs Shared) as a type-level
/// marker.
pub trait Scope {
  type RcMut<T>: From<T> + Clone + RcDerefMut<Target = T>;
  type RcCell<T: Copy + Eq>: SharedCell<T>;
  type BoxedSubscription: Subscription;
  type BoxedObserver<'a, Item, Err>;
  type BoxedObserverMutRef<'a, Item: 'a, Err>;
}

/// Local scope marker - single-threaded, uses Rc/RefCell
pub struct LocalScope;

/// Shared scope marker - multi-threaded, uses Arc/Mutex
pub struct SharedScope;

impl Scope for LocalScope {
  type RcMut<T> = MutRc<T>;
  type RcCell<T: Copy + Eq> = CellRc<T>;
  type BoxedSubscription = BoxedSubscription;
  type BoxedObserver<'a, Item, Err> = BoxedObserver<'a, Item, Err>;
  type BoxedObserverMutRef<'a, Item: 'a, Err> = BoxedObserverMutRef<'a, Item, Err>;
}

impl Scope for SharedScope {
  type RcMut<T> = MutArc<T>;
  type RcCell<T: Copy + Eq> = CellArc<T>;
  type BoxedSubscription = BoxedSubscriptionSend;
  type BoxedObserver<'a, Item, Err> = BoxedObserverSend<'a, Item, Err>;
  type BoxedObserverMutRef<'a, Item: 'a, Err> = BoxedObserverMutRefSend<'a, Item, Err>;
}

// ==================== Context Trait ====================

/// Context trait: Defines the execution environment and provides scheduling
/// capabilities
///
/// This trait is stateful, carrying a Scheduler that can be accessed and
/// preserved across transformations. It provides the following key methods:
///
/// ## Constructors (Static)
/// - `from_parts`: Create from inner value and scheduler (full control)
/// - `new`: Create with default scheduler
/// - `lift<U>`: Create `With<U>` with default scheduler
///
/// ## Combinators (Instance)
/// - `transform`: Transform inner value (Functor map)
/// - `wrap<U>`: Wrap new value, inheriting scheduler
/// - `swap<U>`: Exchange inner value, returning old value
///
/// ## Destructors
/// - `into_inner`: Consume and return inner value
/// - `into_parts`: Consume and return (inner, scheduler)
pub trait Context: Sized {
  type Scope: Scope;
  type Inner;
  type Scheduler: Clone + Default;

  type RcMut<T>: From<T> + Clone + RcDerefMut<Target = T>;
  type RcCell<T: Copy + Eq>: SharedCell<T>;
  type BoxedSubscription: Subscription;
  type BoxedObserver<'a, Item, Err>;
  type BoxedObserverMutRef<'a, Item: 'a, Err>;

  type BoxedCoreObservable<'a, Item, Err>;
  type BoxedCoreObservableMutRef<'a, Item: 'a, Err>;
  type BoxedCoreObservableClone<'a, Item, Err>;
  type BoxedCoreObservableMutRefClone<'a, Item: 'a, Err>;

  type With<T>: Context<Inner = T, Scope = Self::Scope, Scheduler = Self::Scheduler>;

  // ==================== Constructors (Static) ====================

  /// Create a new Context from its parts (inner value and scheduler).
  ///
  /// This is the most low-level constructor, allowing full control over both
  /// the value and the environment (scheduler).
  fn from_parts(inner: Self::Inner, scheduler: Self::Scheduler) -> Self;

  /// Create a new Context with the given inner value and default scheduler.
  ///
  /// Use this when starting a completely new chain with a new value.
  ///
  /// Example:
  /// ```rust
  /// use rxrust::context::{Context, LocalCtx};
  ///
  /// #[derive(Clone, Default)]
  /// struct MyScheduler;
  ///
  /// let ctx = LocalCtx::<_, MyScheduler>::new(1);
  /// ```
  fn new(inner: Self::Inner) -> Self { Self::from_parts(inner, Self::Scheduler::default()) }

  /// Lift a value into Context with default scheduler, producing
  /// `Self::With<U>`.
  ///
  /// Use this method when you want to create a Context for a *different* value
  /// type `U`, but compatible with the current Context family (same Scheduler
  /// type).
  ///
  /// This is essential for operators that need to create Contexts for
  /// transformed values.
  fn lift<U>(inner: U) -> Self::With<U>;

  // ==================== Combinators (Instance) ====================

  /// Transform the inner value (Functor map), inheriting the scheduler.
  ///
  /// Used inside operators to wrap observers.
  fn transform<U, F>(self, f: F) -> Self::With<U>
  where
    F: FnOnce(Self::Inner) -> U;

  /// Wrap a new value in Context, inheriting the scheduler from self.
  ///
  /// Use this when you want to propagate the execution environment to a new
  /// value.
  fn wrap<U>(&self, inner: U) -> Self::With<U>;

  /// Swap the inner value, returning (old_inner, new_context).
  ///
  /// Used at subscribe boundaries to exchange logic with observer.
  fn swap<U>(self, inner: U) -> (Self::Inner, Self::With<U>);

  // ==================== Accessors ====================

  /// Access the environment scheduler
  fn scheduler(&self) -> &Self::Scheduler;

  /// Access the inner value
  fn inner(&self) -> &Self::Inner;

  /// Access the inner value mutably
  fn inner_mut(&mut self) -> &mut Self::Inner;

  // ==================== Destructors ====================

  /// Consume the context and return the inner value
  fn into_inner(self) -> Self::Inner;

  /// Consume the context and return (inner, scheduler)
  fn into_parts(self) -> (Self::Inner, Self::Scheduler);
}
// ==================== Local Context ====================

/// Local context for single-threaded execution
///
/// Uses Rc-based reference counting and a generic Scheduler.
/// Use the `Local` type alias for the default `LocalScheduler`.
#[derive(Clone)]
pub struct LocalCtx<T, S> {
  pub inner: T,
  pub scheduler: S,
}

/// Shared context for multi-threaded execution
///
/// Uses Arc-based reference counting and a generic Scheduler.
/// Requires the scheduler to be Send-safe.
/// Use the `Shared` type alias for the default `SharedScheduler`.
#[derive(Clone)]
pub struct SharedCtx<T, S> {
  pub inner: T,
  pub scheduler: S,
}

// ====================================
// ## Custom Scheduler & Context Implementations
//
// The `LocalCtx` and `SharedCtx` structs are generic over a scheduler type
// `S`. By default, `rxrust::prelude::Local` and `rxrust::prelude::Shared`
// are type aliases that use `LocalCtx<T, LocalScheduler>` and `SharedCtx<T,
// SharedScheduler>` respectively.
//
// Users can inject custom schedulers without implementing the entire `Context`
// trait by defining their own type aliases.
//
// ### Example: Custom `Local` Context with a Custom Scheduler
//
// ```rust,no_run
// use rxrust::{
//   prelude::*,
//   scheduler::{Duration, Schedulable, Scheduler, SleepProvider, TaskHandle},
// };
//
// // 1. Define your custom scheduler (must implement `Scheduler` and `Default`)
// #[derive(Clone, Copy, Default)]
// pub struct MyCustomScheduler;
//
// // 1.1 Provide sleep capability so rxrust's internal `Task<...>` becomes schedulable
// //     automatically via rxrust's generic TaskFuture.
// impl SleepProvider for MyCustomScheduler {
//   type SleepFuture = std::future::Ready<()>;
//
//   fn sleep(&self, _duration: Duration) -> Self::SleepFuture { std::future::ready(()) }
// }
//
// impl<S> Scheduler<S> for MyCustomScheduler
// where
//   S: 'static + Schedulable<MyCustomScheduler>,
// {
//   fn schedule(&self, source: S, _delay: Option<Duration>) -> TaskHandle {
//     let _future = source.into_future(self);
//     TaskHandle::finished()
//   }
// }
//
// // 2. Define a type alias for `LocalCtx` (or `SharedCtx`) using your custom
// //    scheduler. This effectively replaces the default scheduler for this
// //    alias.
// type MyLocal<T> = LocalCtx<T, MyCustomScheduler>;
//
// // Now, `MyLocal` can be used just like `Local` to create observables.
// fn main() {
//   MyLocal::of(10)
//     .map(|v| v * 2)
//     // This delay will be ignored by MyCustomScheduler
//     .delay(Duration::from_secs(999))
//     .subscribe(|v| println!("Received: {}", v));
// }
// // Output:
// // [MyCustomScheduler] Executing task immediately
// // Received: 20
// ```
// ====================================

// Macro to implement `Context` for generic container types like `LocalCtx<T,
// S>`.
macro_rules! impl_context_for_container {
  (
    $Struct:ident,
    $ScopeType:ty,
    $RcMutType:ident,
    $RcCellType:ident,
    $BoxedObserver:ident,
    $BoxedObserverMutRef:ident,
    $BoxedSubscription:ty,
    $BoxedCoreObservable:ty,
    $BoxedCoreObservableMutRef:ty,
    $BoxedCoreObservableClone:ty,
    $BoxedCoreObservableMutRefClone:ty
  ) => {
    impl<T, S> Context for $Struct<T, S>
    where
      S: Clone + Default,
    {
      type Scope = $ScopeType;
      type Inner = T;
      type Scheduler = S;
      type RcMut<U> = $RcMutType<U>;
      type RcCell<U: Copy + Eq> = $RcCellType<U>;
      type With<U> = $Struct<U, S>;
      type BoxedObserver<'a, Item, Err> = $BoxedObserver<'a, Item, Err>;
      type BoxedObserverMutRef<'a, Item: 'a, Err> = $BoxedObserverMutRef<'a, Item, Err>;
      type BoxedSubscription = $BoxedSubscription;
      type BoxedCoreObservable<'a, Item, Err> = $BoxedCoreObservable;
      type BoxedCoreObservableMutRef<'a, Item: 'a, Err> = $BoxedCoreObservableMutRef;
      type BoxedCoreObservableClone<'a, Item, Err> = $BoxedCoreObservableClone;
      type BoxedCoreObservableMutRefClone<'a, Item: 'a, Err> = $BoxedCoreObservableMutRefClone;

      fn from_parts(inner: T, scheduler: S) -> $Struct<T, S> { $Struct { inner, scheduler } }

      fn lift<U>(inner: U) -> $Struct<U, S> { $Struct { inner, scheduler: S::default() } }

      fn scheduler(&self) -> &S { &self.scheduler }

      fn inner(&self) -> &T { &self.inner }
      fn inner_mut(&mut self) -> &mut T { &mut self.inner }

      fn into_inner(self) -> Self::Inner { self.inner }

      fn into_parts(self) -> (Self::Inner, Self::Scheduler) { (self.inner, self.scheduler) }

      fn transform<U, F>(self, f: F) -> $Struct<U, S>
      where
        F: FnOnce(T) -> U,
      {
        $Struct { inner: f(self.inner), scheduler: self.scheduler }
      }

      fn wrap<U>(&self, inner: U) -> $Struct<U, S> {
        $Struct { inner, scheduler: self.scheduler.clone() }
      }

      fn swap<U>(self, new_inner: U) -> (T, $Struct<U, S>) {
        (self.inner, $Struct { inner: new_inner, scheduler: self.scheduler })
      }
    }
  };
}

impl_context_for_container!(
  LocalCtx,
  LocalScope,
  MutRc,
  CellRc,
  BoxedObserver,
  BoxedObserverMutRef,
  BoxedSubscription,
  crate::observable::boxed::BoxedCoreObservable<'a, Item, Err, S>,
  crate::observable::boxed::BoxedCoreObservableMutRef<'a, Item, Err, S>,
  crate::observable::boxed::BoxedCoreObservableClone<'a, Item, Err, S>,
  crate::observable::boxed::BoxedCoreObservableMutRefClone<'a, Item, Err, S>
);
impl_context_for_container!(
  SharedCtx,
  SharedScope,
  MutArc,
  CellArc,
  BoxedObserverSend,
  BoxedObserverMutRefSend,
  BoxedSubscriptionSend,
  crate::observable::boxed::BoxedCoreObservableSend<'a, Item, Err, S>,
  crate::observable::boxed::BoxedCoreObservableMutRefSend<'a, Item, Err, S>,
  crate::observable::boxed::BoxedCoreObservableSendClone<'a, Item, Err, S>,
  crate::observable::boxed::BoxedCoreObservableMutRefSendClone<'a, Item, Err, S>
);

// ==================== Type Aliases ====================

/// These type aliases provide convenient defaults for `LocalCtx` and
/// `SharedCtx`.
///
/// Tip: You can define your own aliases to inject a custom scheduler without
/// implementing `Context` yourself. For example:
///
/// ```rust
/// use rxrust::context::LocalCtx;
/// // Your scheduler type implements `Clone + Default` and `Scheduler`
/// #[derive(Clone, Default)]
/// struct MyScheduler;
/// // Define a custom local alias that uses `MyScheduler`
/// type MyLocal<T> = LocalCtx<T, MyScheduler>;
/// ```
///
/// This lets you write `MyLocal::of(1).map(...).subscribe(...)` with your
/// scheduler plugged into the pipeline.
#[cfg(feature = "scheduler")]
pub type Local<T> = LocalCtx<T, LocalScheduler>;

#[cfg(feature = "scheduler")]
pub type Shared<T> = SharedCtx<T, SharedScheduler>;

/// Test context for deterministic time-based testing.
///
/// This is a type alias for `LocalCtx<T, TestScheduler>`, providing a
/// single-threaded context that uses virtual time instead of real time. All
/// `TestCtx` instances in the same thread share the same virtual time and task
/// queue via thread-local storage.
///
/// # Usage
///
/// ```rust
/// use rxrust::{prelude::*, scheduler::test_scheduler::TestScheduler};
///
/// // Initialize the test scheduler (required before use)
/// TestScheduler::init();
///
/// // Create observables using TestCtx
/// let received = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
/// let received_clone = received.clone();
///
/// TestCtx::of(42)
///   .delay(Duration::from_millis(100))
///   .subscribe(move |v| received_clone.borrow_mut().push(v));
///
/// // Value not received yet (virtual time is 0)
/// assert!(received.borrow().is_empty());
///
/// // Advance virtual time to trigger the delayed emission
/// TestScheduler::advance_by(Duration::from_millis(100));
///
/// // Now the value should be received
/// assert_eq!(*received.borrow(), vec![42]);
/// ```
///
/// # Thread Safety
///
/// `TestCtx` uses `Rc`-based reference counting (like `Local`), so it does not
/// require `Send` bounds on task state. This makes it suitable for testing
/// single-threaded code with `!Send` types.
#[cfg(test)]
pub type TestCtx<T> = LocalCtx<T, crate::scheduler::test_scheduler::TestScheduler>;

// ==================== Shared Context ====================

// Macro to implement `Observer` and `Subscription` for container contexts.
macro_rules! impl_observer_and_subscription {
  ($Struct:ident $(+ $Bound:ident)?) => {
    impl<
      Item,
      Err,
      S,
      Inner: Observer<Item, Err> $(+ $Bound)?
    > Observer<Item, Err> for $Struct<Inner, S>
    where
      S: Clone + Default,
    {
      fn next(&mut self, value: Item) { self.inner.next(value); }
      fn error(self, err: Err) { self.inner.error(err); }
      fn complete(self) { self.inner.complete(); }
      fn is_closed(&self) -> bool { self.inner.is_closed() }
    }

    impl<S, Inner: Subscription $(+ $Bound)? > Subscription for $Struct<Inner, S>
    where
      S: Clone + Default,
    {
      fn unsubscribe(self) { self.inner.unsubscribe(); }
      fn is_closed(&self) -> bool { self.inner.is_closed() }
    }
  };
}

// Apply macros for `LocalCtx` (no extra bounds) and `SharedCtx` (requires
// `Send`).
impl_observer_and_subscription!(LocalCtx);
impl_observer_and_subscription!(SharedCtx + Send);

#[cfg(feature = "scheduler")]
impl<T, S> LocalCtx<T, S>
where
  T: Send + 'static, // T needs to be Send for SharedCtx
  S: Clone + Default,
{
  /// Converts this `LocalCtx` into a `Shared` context,
  /// consuming the original `LocalCtx`.
  /// The inner value `T` must be `Send + 'static` to be compatible with
  /// `SharedCtx`. The scheduler of the new `Shared` context will be
  /// `SharedScheduler::default()`.
  pub fn into_shared(self) -> Shared<T> { Shared::new(self.into_inner()) }
}

#[cfg(feature = "scheduler")]
impl<T, S> SharedCtx<T, S>
where
  S: Clone + Default,
{
  /// Converts this `SharedCtx` into a `Local` context,
  /// consuming the original `SharedCtx`.
  /// The scheduler of the new `Local` context will be
  /// `LocalScheduler::default()`.
  pub fn into_local(self) -> Local<T> { Local::new(self.into_inner()) }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    factory::ObservableFactory,
    observable::Observable,
    scheduler::{Duration, Task, TaskHandle},
  };

  // A minimal custom scheduler for testing the shadowing mechanism
  #[derive(Clone, Copy, Default)]
  struct CustomTestScheduler;

  impl<S> crate::scheduler::Schedulable<CustomTestScheduler> for Task<S> {
    type Future = std::future::Ready<()>;
    fn into_future(self, _scheduler: &CustomTestScheduler) -> Self::Future {
      std::future::ready(())
    }
  }

  impl<S> crate::scheduler::Scheduler<Task<S>> for CustomTestScheduler {
    fn schedule(&self, mut task: Task<S>, _delay: Option<Duration>) -> TaskHandle {
      // In a test, we can just run it immediately for simplicity
      task.step();
      TaskHandle::finished()
    }
  }

  // A custom context struct to demonstrate using a custom scheduler.
  // Since `Local` is now bound to `LocalScheduler`, we need our own struct
  // to carry a `CustomTestScheduler`.
  #[derive(Clone)]
  struct CustomContext<T> {
    inner: T,
    scheduler: CustomTestScheduler,
  }

  impl<T> Context for CustomContext<T> {
    type Scope = LocalScope;
    type Inner = T;
    type Scheduler = CustomTestScheduler;
    type RcMut<U> = MutRc<U>;
    type RcCell<U: Copy + Eq> = CellRc<U>;
    type With<U> = CustomContext<U>;
    type BoxedObserver<'a, Item, Err> = BoxedObserver<'a, Item, Err>;
    type BoxedObserverMutRef<'a, Item: 'a, Err> = BoxedObserverMutRef<'a, Item, Err>;
    type BoxedSubscription = BoxedSubscription;
    type BoxedCoreObservable<'a, Item, Err> =
      crate::observable::boxed::BoxedCoreObservable<'a, Item, Err, CustomTestScheduler>;
    type BoxedCoreObservableMutRef<'a, Item: 'a, Err> =
      crate::observable::boxed::BoxedCoreObservableMutRef<'a, Item, Err, CustomTestScheduler>;
    type BoxedCoreObservableClone<'a, Item, Err> =
      crate::observable::boxed::BoxedCoreObservableClone<'a, Item, Err, CustomTestScheduler>;
    type BoxedCoreObservableMutRefClone<'a, Item: 'a, Err> =
      crate::observable::boxed::BoxedCoreObservableMutRefClone<'a, Item, Err, CustomTestScheduler>;

    fn from_parts(inner: T, scheduler: CustomTestScheduler) -> CustomContext<T> {
      CustomContext { inner, scheduler }
    }

    fn lift<U>(inner: U) -> CustomContext<U> {
      CustomContext { inner, scheduler: CustomTestScheduler }
    }

    fn scheduler(&self) -> &CustomTestScheduler { &self.scheduler }

    fn inner(&self) -> &T { &self.inner }

    fn inner_mut(&mut self) -> &mut T { &mut self.inner }

    fn transform<U, F>(self, f: F) -> CustomContext<U>
    where
      F: FnOnce(T) -> U,
    {
      CustomContext { inner: f(self.inner), scheduler: self.scheduler }
    }

    fn wrap<U>(&self, inner: U) -> CustomContext<U> {
      CustomContext { inner, scheduler: self.scheduler }
    }

    fn swap<U>(self, new_inner: U) -> (T, CustomContext<U>) {
      (self.inner, CustomContext { inner: new_inner, scheduler: self.scheduler })
    }

    fn into_inner(self) -> Self::Inner { self.inner }

    fn into_parts(self) -> (Self::Inner, Self::Scheduler) { (self.inner, self.scheduler) }
  }

  #[rxrust_macro::test]
  fn test_factory_blanket_impl_with_defaults() {
    // These should compile due to the blanket implementation using default
    // schedulers
    let _local_of = Local::of(1);
    let _shared_of = Shared::of(2);
  }

  #[rxrust_macro::test]
  fn test_factory_blanket_impl_with_custom_scheduler() {
    // This should compile, demonstrating custom scheduler injection
    let _custom_local_of = CustomContext::of("hello");
  }

  #[rxrust_macro::test]
  fn test_custom_scheduler_type_alias() {
    // 1. Define custom scheduler
    #[derive(Clone, Copy, Default)]
    struct MyScheduler;

    impl<S> crate::scheduler::Schedulable<MyScheduler> for Task<S> {
      type Future = std::future::Ready<()>;
      fn into_future(self, _scheduler: &MyScheduler) -> Self::Future { std::future::ready(()) }
    }

    impl<S> crate::scheduler::Scheduler<Task<S>> for MyScheduler {
      fn schedule(
        &self, mut task: Task<S>, _delay: Option<Duration>,
      ) -> crate::scheduler::TaskHandle {
        task.step();
        crate::scheduler::TaskHandle::finished()
      }
    }

    // 2. Define alias
    type MyLocal<T> = LocalCtx<T, MyScheduler>;

    // 3. Use it
    let executed = std::rc::Rc::new(std::cell::RefCell::new(false));
    let exec_clone = executed.clone();

    MyLocal::of(123)
      .delay(Duration::from_secs(1))
      .subscribe(move |v| {
        assert_eq!(v, 123);
        *exec_clone.borrow_mut() = true;
      });

    assert!(*executed.borrow());
  }
}

#[cfg(test)]
mod context_conversion_tests {
  use super::*;

  #[rxrust_macro::test]
  fn test_local_to_shared_conversion() {
    let local_ctx = Local::new(100);
    assert_eq!(*local_ctx.inner(), 100);

    let shared_ctx = local_ctx.into_shared();
    assert_eq!(*shared_ctx.inner(), 100);
    // Verify the type has changed
    let _: Shared<i32> = shared_ctx;
  }

  #[rxrust_macro::test]
  fn test_shared_to_local_conversion() {
    // Need to start with a Shared context
    let shared_ctx = Shared::new(200);
    assert_eq!(*shared_ctx.inner(), 200);

    let local_ctx = shared_ctx.into_local();
    assert_eq!(*local_ctx.inner(), 200);
    // Verify the type has changed
    let _: Local<i32> = local_ctx;
  }

  #[rxrust_macro::test]
  fn test_round_trip_conversion() {
    let original_local = Local::new(300);
    assert_eq!(*original_local.inner(), 300);

    let shared_from_local = original_local.into_shared();
    assert_eq!(*shared_from_local.inner(), 300);

    let local_from_shared = shared_from_local.into_local();
    assert_eq!(*local_from_shared.inner(), 300);

    // Can't directly compare schedulers because they are new defaults
    // But we can assert the types are correct
    let _: Local<i32> = local_from_shared;
  }
}
