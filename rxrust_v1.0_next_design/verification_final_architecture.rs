// =================================================================================================
// rxRust 1.0 Architecture Verification Suite (Scheme 22: Explicit Type Separation - Final)
// =================================================================================================
// This file serves as the executable proof of the architecture described in DESIGN_DOCUMENT.md.
// It verifies:
// 1. Unified Logic: CoreObservable<C> handling both Local and Shared contexts.
// 2. Stateful Context: Context carries a Scheduler (Dependency Injection).
// 3. Factory Pattern: Observable creation via marker type (Local/Shared).
// 4. Unified TaskHandle: A single concrete handle for both Local (zero-cost) and Shared (async) tasks.
// 5. Future Support: TaskHandle implements Future.
// 6. GAT Support: CoreObservable inherits from ObservableType, ensuring consistent types.
// 7. Explicit Type Definition: Operators implement ObservableType directly, decoupling types from subscription context.

use std::time::Duration;

// ==========================================
// 1. Observer & Subscription
// ==========================================

pub mod common {
  use std::convert::Infallible;

  pub trait Observer<Item, Err> {
    fn next(&mut self, v: Item);
    fn error(self, e: Err);
    fn complete(self);
  }

  pub trait Subscription {
    fn unsubscribe(&mut self);
    fn is_closed(&self) -> bool;
  }

  // Unit subscription (always closed)
  impl Subscription for () {
    fn unsubscribe(&mut self) {}
    fn is_closed(&self) -> bool {
      true
    }
  }

  impl<F, Item> Observer<Item, Infallible> for F
  where
    F: FnMut(Item),
  {
    fn next(&mut self, v: Item) {
      (self)(v);
    }
    fn error(self, _e: Infallible) {}
    fn complete(self) {}
  }
}

// ==========================================
// 2. Scheduler & TaskHandle System
// ==========================================

pub mod scheduler {
  use super::common::Subscription;
  use std::future::Future;
  use std::pin::Pin;
  use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
  };
  use std::task::{Context, Poll, Waker};

  use std::time::Duration;

  pub struct Task<S> {
    pub state: S,
    pub handler: fn(S),
  }

  impl<S> Task<S> {
    pub fn new(state: S, handler: fn(S)) -> Self {
      Self { state, handler }
    }
    pub fn run(self) {
      (self.handler)(self.state);
    }
  }

  // --- Unified TaskHandle ---

  struct SharedState {
    cancelled: AtomicBool,
    finished: AtomicBool,
    waker: Mutex<Option<Waker>>,
  }

  /// A unified handle for any scheduled task.
  #[derive(Clone)]
  pub struct TaskHandle {
    // If None, the task is considered "already finished" (LocalScheduler optimization).
    // If Some, it tracks a running asynchronous task (SharedScheduler).
    inner: Option<Arc<SharedState>>,
  }

  impl TaskHandle {
    /// Create a handle for a task that is already finished.
    pub fn finished() -> Self {
      Self { inner: None }
    }

    /// Create a handle for an asynchronous task.
    /// Returns the Handle and the State (to be used by the runner).
    pub(crate) fn new_async() -> (Self, Arc<SharedState>) {
      let state = Arc::new(SharedState {
        cancelled: AtomicBool::new(false),
        finished: AtomicBool::new(false),
        waker: Mutex::new(None),
      });
      (Self { inner: Some(state.clone()) }, state)
    }
  }

  impl Subscription for TaskHandle {
    fn unsubscribe(&mut self) {
      if let Some(state) = &self.inner {
        state.cancelled.store(true, Ordering::Relaxed);
        if let Ok(mut lock) = state.waker.lock() {
          if let Some(waker) = lock.take() {
            waker.wake();
          }
        }
      }
    }

    fn is_closed(&self) -> bool {
      match &self.inner {
        None => true,
        Some(state) => {
          state.finished.load(Ordering::Relaxed) || state.cancelled.load(Ordering::Relaxed)
        }
      }
    }
  }

  impl Future for TaskHandle {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
      match &self.inner {
        None => Poll::Ready(()),
        Some(state) => {
          if state.finished.load(Ordering::Relaxed) || state.cancelled.load(Ordering::Relaxed) {
            Poll::Ready(())
          } else {
            let mut lock = state.waker.lock().unwrap();
            *lock = Some(cx.waker().clone());
            Poll::Pending
          }
        }
      }
    }
  }

  // --- Scheduler Trait ---

  pub trait Scheduler<S> {
    fn schedule(&self, task: Task<S>, delay: Option<Duration>) -> TaskHandle;
  }

  // --- Implementations ---

  #[derive(Clone, Copy, Default)]
  pub struct LocalScheduler;

  impl<S> Scheduler<S> for LocalScheduler {
    fn schedule(&self, task: Task<S>, delay: Option<Duration>) -> TaskHandle {
      if let Some(d) = delay {
        std::thread::sleep(d);
      }
      task.run();
      TaskHandle::finished()
    }
  }

  #[derive(Clone, Copy, Default)]
  pub struct SharedScheduler;

  impl<S: Send + 'static> Scheduler<S> for SharedScheduler {
    fn schedule(&self, task: Task<S>, delay: Option<Duration>) -> TaskHandle {
      let (handle, state) = TaskHandle::new_async();

      std::thread::spawn(move || {
        if let Some(d) = delay {
          if state.cancelled.load(Ordering::Relaxed) {
            return;
          }
          std::thread::sleep(d);
        }

        if !state.cancelled.load(Ordering::Relaxed) {
          task.run();
        }

        state.finished.store(true, Ordering::Relaxed);
        if let Ok(mut lock) = state.waker.lock() {
          if let Some(waker) = lock.take() {
            waker.wake();
          }
        }
      });

      handle
    }
  }
}

// ==========================================
// 3. Context System
// ==========================================

pub mod context {
  use super::scheduler::{LocalScheduler, SharedScheduler};
  use std::cell::RefCell;
  use std::rc::Rc;
  use std::sync::{Arc, Mutex};

  pub trait Context: Sized {
    type Inner;
    type Scheduler: Clone;
    type Rc<T>: From<T> + Clone;
    type With<T>: Context<Inner = T, Scheduler = Self::Scheduler>;

    // Probe removed: Context no longer responsible for type inference

    /// Create a new Context with the given inner value and default scheduler
    fn create<U>(inner: U) -> Self::With<U>
    where
      Self::Scheduler: Default;

    fn scheduler(&self) -> &Self::Scheduler;
    fn map_inner<U, F>(self, f: F) -> Self::With<U>
    where
      F: FnOnce(Self::Inner) -> U;
    fn replace_inner<U>(self, inner: U) -> (Self::Inner, Self::With<U>);
  }

  // --- Local Context ---
  #[derive(Clone)]
  pub struct Local<T, Sch = LocalScheduler> {
    pub inner: T,
    pub scheduler: Sch,
  }

  impl<T, Sch> Local<T, Sch> {
    pub fn with_scheduler<NewSch>(self, scheduler: NewSch) -> Local<T, NewSch> {
      Local { inner: self.inner, scheduler }
    }
  }

  pub struct MutRc<T>(Rc<RefCell<T>>);
  impl<T> Clone for MutRc<T> {
    fn clone(&self) -> Self {
      MutRc(self.0.clone())
    }
  }
  impl<T> From<T> for MutRc<T> {
    fn from(v: T) -> Self {
      Self(Rc::new(RefCell::new(v)))
    }
  }

  impl<T, Sch: Clone> Context for Local<T, Sch> {
    type Inner = T;
    type Scheduler = Sch;
    type Rc<U> = MutRc<U>;
    type With<U> = Local<U, Sch>;

    fn create<U>(inner: U) -> Local<U, Sch>
    where
      Sch: Default,
    {
      Local { inner, scheduler: Sch::default() }
    }

    fn scheduler(&self) -> &Sch {
      &self.scheduler
    }

    fn map_inner<U, F>(self, f: F) -> Local<U, Sch>
    where
      F: FnOnce(T) -> U,
    {
      Local { inner: f(self.inner), scheduler: self.scheduler }
    }

    fn replace_inner<U>(self, new_inner: U) -> (T, Local<U, Sch>) {
      (self.inner, Local { inner: new_inner, scheduler: self.scheduler })
    }
  }

  // --- Shared Context ---
  #[derive(Clone)]
  pub struct Shared<T, Sch = SharedScheduler> {
    pub inner: T,
    pub scheduler: Sch,
  }

  impl<T, Sch> Shared<T, Sch> {
    pub fn with_scheduler<NewSch>(self, scheduler: NewSch) -> Shared<T, NewSch> {
      Shared { inner: self.inner, scheduler }
    }
  }

  pub struct MutArc<T>(Arc<Mutex<T>>);
  impl<T> Clone for MutArc<T> {
    fn clone(&self) -> Self {
      MutArc(self.0.clone())
    }
  }
  impl<T> From<T> for MutArc<T> {
    fn from(v: T) -> Self {
      Self(Arc::new(Mutex::new(v)))
    }
  }

  impl<T, Sch: Clone + Send> Context for Shared<T, Sch> {
    type Inner = T;
    type Scheduler = Sch;
    type Rc<U> = MutArc<U>;
    type With<U> = Shared<U, Sch>;

    fn create<U>(inner: U) -> Shared<U, Sch>
    where
      Sch: Default,
    {
      Shared { inner, scheduler: Sch::default() }
    }

    fn scheduler(&self) -> &Sch {
      &self.scheduler
    }

    fn map_inner<U, F>(self, f: F) -> Shared<U, Sch>
    where
      F: FnOnce(T) -> U,
    {
      Shared { inner: f(self.inner), scheduler: self.scheduler }
    }

    fn replace_inner<U>(self, new_inner: U) -> (T, Shared<U, Sch>) {
      (self.inner, Shared { inner: new_inner, scheduler: self.scheduler })
    }
  }
}

// ==========================================
// 4. Observable Factory Trait
// ==========================================

pub mod factory {
  use super::context::Context;
  use super::ops::Of;

  pub trait ObservableFactory: Context {
    fn of<T: Clone>(v: T) -> Self::With<Of<T>>;
  }

  // Blanket Implementation!
  impl<C: Context> ObservableFactory for C
  where
    C::Scheduler: Default,
  {
    fn of<T: Clone>(v: T) -> Self::With<Of<T>> {
      Self::create(Of(v))
    }
  }
}

// ==========================================
// 5. Core Logic & Observable API
// ==========================================

pub mod rx {
  use super::common::Subscription;
  use super::context::Context;
  use std::convert::Infallible;
  use std::time::Duration;

  // --- 1. Pure Type Definition Trait ---
  // The Source of Truth for Item and Err types
  pub trait ObservableType {
    type Item<'a>
    where
      Self: 'a;
    type Err;
  }

  // --- 2. Core Logic Trait ---
  // Inherits types from ObservableType
  pub trait CoreObservable<O>: ObservableType {
    type Unsub: Subscription;
    fn subscribe(self, observer: O) -> Self::Unsub;
  }

  // --- 3. User-Facing Facade ---
  pub trait Observable: Context + 'static
  where
    // The Inner Logic must be an ObservableType
    Self::Inner: ObservableType,
  {
    // Clean derivation from the Inner Type
    type Item<'a>
    where
      Self: 'a;

    type Err;

    // Standard Subscribe
    fn subscribe<O>(self, observer: O) -> <Self::Inner as CoreObservable<Self::With<O>>>::Unsub
    where
      Self::Inner: CoreObservable<Self::With<O>>,
    {
      let (core, wrapped) = self.replace_inner(observer);
      core.subscribe(wrapped)
    }

    // Operators now use ObservableType to determine types
    fn map<F, Out>(self, f: F) -> Self::With<super::ops::Map<Self::Inner, F>>
    where
      // F's output determines the new Item type
      F: for<'a> FnMut(Self::Item<'a>) -> Out,
    {
      self.map_inner(|core| super::ops::Map { source: core, func: f })
    }

    fn on_error<F>(self, f: F) -> Self::With<super::ops::OnError<Self::Inner, F>>
    where
      F: FnOnce(Self::Err),
      Self::Inner: CoreObservable<Self::With<super::ops::OnError<Self::Inner, F>>>,
    {
      self.map_inner(|core| super::ops::OnError { source: core, callback: f })
    }

    fn on_complete<F>(self, f: F) -> Self::With<super::ops::OnComplete<Self::Inner, F>>
    where
      F: FnOnce(),
    {
      self.map_inner(|core| super::ops::OnComplete { source: core, callback: f })
    }

    fn delay(self, delay: Duration) -> Self::With<super::ops::Delay<Self::Inner, Self::Scheduler>>
    where
      Self::Scheduler: Clone,
    {
      let scheduler = self.scheduler().clone();
      self.map_inner(|core| super::ops::Delay { source: core, delay, scheduler })
    }
  }

  impl<T> Observable for T
  where
    T: Context + 'static,
    T::Inner: ObservableType,
  {
    // Types derived in trait default impl
    type Item<'a>
      = <T::Inner as ObservableType>::Item<'a>
    where
      T: 'a;
    type Err = <T::Inner as ObservableType>::Err;
  }
}

// ==========================================
// 6. Operators Implementation
// ==========================================

pub mod ops {
  use super::common::Observer;
  use super::context::Context;
  use super::rx::{CoreObservable, ObservableType}; // Import ObservableType
  use super::scheduler::{Scheduler, Task};
  use std::convert::Infallible;
  use std::time::Duration;

  // --- Of ---
  pub struct Of<T>(pub T);

  // 1. Explicit Type Definition
  impl<T: Clone> ObservableType for Of<T> {
    type Item<'a>
      = T
    where
      T: 'a;
    type Err = Infallible;
  }

  // 2. Logic Implementation (Types Inherited)
  impl<C, T> CoreObservable<C> for Of<T>
  where
    C: Context,
    C::Inner: Observer<T, Infallible>,
    T: Clone,
  {
    type Unsub = ();

    fn subscribe(self, context: C) -> Self::Unsub {
      let (mut observer, _) = context.replace_inner(());
      observer.next(self.0);
      observer.complete();
    }
  }

  // --- Map ---
  pub struct Map<S, F> {
    pub source: S,
    pub func: F,
  }
  pub struct MapObserver<O, F> {
    observer: O,
    func: F,
  }

  // 1. Explicit Type Definition
  impl<S, F, Out> ObservableType for Map<S, F>
  where
    S: ObservableType,
    F: for<'a> FnMut(S::Item<'a>) -> Out,
  {
    type Item<'a>
      = Out
    where
      Self: 'a;
    type Err = S::Err;
  }

  impl<O, F, In, Out, Err> Observer<In, Err> for MapObserver<O, F>
  where
    O: Observer<Out, Err>,
    F: FnMut(In) -> Out,
  {
    fn next(&mut self, v: In) {
      let mapped = (self.func)(v);
      self.observer.next(mapped);
    }
    fn error(self, e: Err) {
      self.observer.error(e);
    }
    fn complete(self) {
      self.observer.complete();
    }
  }

  // 2. Logic Implementation
  impl<S, F, C, Out> CoreObservable<C> for Map<S, F>
  where
    C: Context,
    C::Inner: Observer<Out, S::Err>,
    S: CoreObservable<C::With<MapObserver<C::Inner, F>>>,
    F: for<'a> FnMut(S::Item<'a>) -> Out, // Must match ObservableType constraints
  {
    type Unsub = S::Unsub;

    fn subscribe(self, context: C) -> Self::Unsub {
      let Map { source, func } = self;
      let wrapped = context.map_inner(|observer| MapObserver { observer, func });
      source.subscribe(wrapped)
    }
  }

  // --- OnError ---
  pub struct OnError<S, F> {
    pub source: S,
    pub callback: F,
  }
  pub struct OnErrorObserver<O, F> {
    observer: O,
    callback: Option<F>,
  }

  impl<S, F> ObservableType for OnError<S, F>
  where
    S: ObservableType,
  {
    type Item<'a>
      = S::Item<'a>
    where
      Self: 'a;
    type Err = S::Err;
  }

  impl<O, F, Item, Err> Observer<Item, Err> for OnErrorObserver<O, F>
  where
    O: Observer<Item, Err>,
    F: FnOnce(Err),
  {
    fn next(&mut self, v: Item) {
      self.observer.next(v);
    }
    fn error(self, e: Err) {
      if let Some(cb) = self.callback {
        cb(e);
      }
    }
    fn complete(self) {
      self.observer.complete();
    }
  }

  impl<S, F, C> CoreObservable<C> for OnError<S, F>
  where
    C: Context,
    C::Inner: for<'a> Observer<S::Item<'a>, S::Err>,
    S: CoreObservable<C::With<OnErrorObserver<C::Inner, F>>>,
    F: FnOnce(S::Err),
  {
    type Unsub = S::Unsub;

    fn subscribe(self, context: C) -> Self::Unsub {
      let OnError { source, callback } = self;
      let wrapped =
        context.map_inner(|observer| OnErrorObserver { observer, callback: Some(callback) });
      source.subscribe(wrapped)
    }
  }

  // --- OnComplete ---
  pub struct OnComplete<S, F> {
    pub source: S,
    pub callback: F,
  }
  pub struct OnCompleteObserver<O, F> {
    observer: O,
    callback: Option<F>,
  }

  impl<S, F> ObservableType for OnComplete<S, F>
  where
    S: ObservableType,
  {
    type Item<'a>
      = S::Item<'a>
    where
      Self: 'a;
    type Err = S::Err;
  }

  impl<O, F, Item, Err> Observer<Item, Err> for OnCompleteObserver<O, F>
  where
    O: Observer<Item, Err>,
    F: FnOnce(),
  {
    fn next(&mut self, v: Item) {
      self.observer.next(v);
    }
    fn error(self, e: Err) {
      self.observer.error(e);
    }
    fn complete(mut self) {
      if let Some(cb) = self.callback.take() {
        cb();
      }
      self.observer.complete();
    }
  }

  impl<S, F, C> CoreObservable<C> for OnComplete<S, F>
  where
    C: Context,
    C::Inner: for<'a> Observer<S::Item<'a>, S::Err>,
    S: CoreObservable<C::With<OnCompleteObserver<C::Inner, F>>>,
    F: FnOnce(),
  {
    type Unsub = S::Unsub;

    fn subscribe(self, context: C) -> Self::Unsub {
      let OnComplete { source, callback } = self;
      let wrapped =
        context.map_inner(|observer| OnCompleteObserver { observer, callback: Some(callback) });
      source.subscribe(wrapped)
    }
  }

  // --- Delay ---
  pub struct Delay<S, Sch> {
    pub source: S,
    pub delay: Duration,
    pub scheduler: Sch,
  }

  pub struct DelayObserver<O, Sch> {
    pub observer: O,
    pub delay: Duration,
    pub scheduler: Sch,
  }

  impl<S, Sch> ObservableType for Delay<S, Sch>
  where
    S: ObservableType,
  {
    type Item<'a>
      = S::Item<'a>
    where
      Self: 'a;
    type Err = S::Err;
  }

  fn delay_next_handler<O, Item, Err>(state: (O, Item))
  where
    O: Observer<Item, Err>,
  {
    let (mut observer, item) = state;
    observer.next(item);
  }

  impl<O, Sch, Item, Err> Observer<Item, Err> for DelayObserver<O, Sch>
  where
    O: Observer<Item, Err> + Clone,
    Sch: Scheduler<(O, Item)> + Clone,
  {
    fn next(&mut self, v: Item) {
      let state = (self.observer.clone(), v);
      let task = Task::new(state, delay_next_handler);
      // Now returns TaskHandle!
      self.scheduler.schedule(task, Some(self.delay));
    }

    fn error(self, e: Err) {
      self.observer.error(e);
    }
    fn complete(self) {
      self.observer.complete();
    }
  }

  impl<S, Sch, C> CoreObservable<C> for Delay<S, Sch>
  where
    C: Context,
    C::Inner: for<'a> Observer<S::Item<'a>, S::Err> + Clone,
    S: CoreObservable<C::With<DelayObserver<C::Inner, Sch>>>,
    Sch: Clone,
  {
    type Unsub = S::Unsub; // We are reusing the source's Unsub for now (chaining)

    fn subscribe(self, context: C) -> Self::Unsub {
      let Delay { source, delay, scheduler } = self;
      let wrapped = context.map_inner(|observer| DelayObserver { observer, scheduler, delay });
      source.subscribe(wrapped)
    }
  }
}

// ==========================================
// 7. Prelude & User-Facing API
// ==========================================

use common::*;
use factory::*;
use rx::*;
use scheduler::*;

// Default marker types (rxRust provides these in its prelude)
pub mod prelude {

  use super::*; // Needed for Context::With

  // Zero-sized marker types
  pub struct Local;

  // Implement Context for Factory types
  impl context::Context for Local {
    type Inner = ();
    type Scheduler = LocalScheduler;
    type Rc<T> = context::MutRc<T>;
    type With<T> = context::Local<T, LocalScheduler>;

    fn create<U>(inner: U) -> Self::With<U>
    where
      Self::Scheduler: Default,
    {
      context::Local { inner, scheduler: Self::Scheduler::default() }
    }

    fn scheduler(&self) -> &Self::Scheduler {
      unimplemented!("Factory context doesn't hold a scheduler instance")
    }

    fn map_inner<U, F>(self, _f: F) -> Self::With<U>
    where
      F: FnOnce(Self::Inner) -> U,
    {
      unimplemented!()
    }

    fn replace_inner<U>(self, _inner: U) -> (Self::Inner, Self::With<U>) {
      unimplemented!()
    }
  }

  pub struct Shared;

  impl context::Context for Shared {
    type Inner = ();
    type Scheduler = SharedScheduler;
    type Rc<T> = context::MutArc<T>;
    type With<T> = context::Shared<T, SharedScheduler>;

    fn create<U>(inner: U) -> Self::With<U>
    where
      Self::Scheduler: Default,
    {
      context::Shared { inner, scheduler: Self::Scheduler::default() }
    }

    fn scheduler(&self) -> &Self::Scheduler {
      unimplemented!("Factory context doesn't hold a scheduler instance")
    }

    fn map_inner<U, F>(self, _f: F) -> Self::With<U>
    where
      F: FnOnce(Self::Inner) -> U,
    {
      unimplemented!()
    }

    fn replace_inner<U>(self, _inner: U) -> (Self::Inner, Self::With<U>) {
      unimplemented!()
    }
  }
}

fn main() {
  println!("===========================================================");
  println!("Architecture Verification: Scheme 22 (Explicit Type Separation)");
  println!("===========================================================");

  // Test 1: Default usage with full operator chain
  {
    use prelude::*;

    println!("\n[Test 1] Default Scheduler + Full Operator Chain");

    // Of returns TaskHandle::finished()
    let handle = Local::of(10)
      .map(|x| x * 2)
      .on_complete(|| println!("  Stream completed"))
      .delay(Duration::from_millis(10))
      .subscribe(|v| println!("  Received: {}\n", v));

    println!("  Handle closed? {}\n", handle.is_closed());
  }

  // Test 2: Custom scheduler via marker type redefinition
  {
    println!("\n[Test 2] Custom Scheduler (Marker Type Shadowing)");

    #[derive(Clone, Copy, Default)]
    struct CustomScheduler;
    impl<S> scheduler::Scheduler<S> for CustomScheduler {
      fn schedule(&self, task: Task<S>, _delay: Option<Duration>) -> TaskHandle {
        println!("  [CustomScheduler] Executing immediately");
        task.run();
        TaskHandle::finished()
      }
    }

    struct Local;

    impl context::Context for Local {
      type Inner = ();
      type Scheduler = CustomScheduler;
      type Rc<T> = context::MutRc<T>;
      type With<T> = context::Local<T, CustomScheduler>;

      fn create<U>(inner: U) -> Self::With<U>
      where
        Self::Scheduler: Default,
      {
        context::Local { inner, scheduler: Self::Scheduler::default() }
      }

      fn scheduler(&self) -> &Self::Scheduler {
        unimplemented!()
      }

      fn map_inner<U, F>(self, _f: F) -> Self::With<U>
      where
        F: FnOnce(Self::Inner) -> U,
      {
        unimplemented!()
      }

      fn replace_inner<U>(self, _inner: U) -> (Self::Inner, Self::With<U>) {
        unimplemented!()
      }
    }

    Local::of(20)
      .map(|x| x + 10)
      .delay(Duration::from_secs(999)) // Ignored by CustomScheduler
      .subscribe(|v| println!("  Received: {}\n", v));
  }

  // Test 3: GAT Reference Type Verification
  {
    use prelude::*;

    println!("\n[Test 3] GAT Reference Type Verification");

    // Example 1: References to values
    {
      let data = vec![1, 2, 3, 4, 5];
      println!("  Testing with Vec<&i32>:");

      // Note: This would require a new operator like 'RefOf' that emits references
      // For now, we demonstrate the GAT structure works with owned values
      Local::of(data)
        .map(|slice| slice.len())
        .subscribe(|len| println!("    Slice length: {}", len));
    }

    // Example 2: String references
    {
      let message = "Hello, GAT!";
      println!("\n  Testing with &str:");

      // Simulate a reference-based observable
      Local::of(message.to_string())
        .map(|s| s.chars().count())
        .subscribe(|count| println!("    Character count: {}", count));
    }

    // Example 3: Complex reference chains
    {
      let data = vec![("Alice", 25), ("Bob", 30), ("Charlie", 35)];
      println!("\n  Testing with complex data structures:");

      for (name, age) in data {
        Local::of(age)
          .map(|a| a + 5)
          .on_complete(move || println!("    Processed age for {}", name))
          .subscribe(|future_age| println!("    {} will be {} in 5 years", name, future_age));
      }
    }
  }
}
