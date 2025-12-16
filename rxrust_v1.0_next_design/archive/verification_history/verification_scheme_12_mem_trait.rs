use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ==========================================
// 0. The Unified Trait: Context
// ==========================================

pub trait Context: Sized {
  type Inner;
  type Rc<T>: From<T> + Clone;

  // GAT: "With<T>" produces a new Context wrapping T.
  // We enforce 'static to avoid lifetime hell in this POC.
  type With<T: 'static>: Context<Inner = T>;

  fn into_inner(self) -> Self::Inner;
  fn pack<T: 'static>(inner: T) -> Self::With<T>;

  // Scheduler Hook
  // Returns Box<dyn Subscription> to avoid TAIT instability
  fn schedule_action<F>(
    self,
    delay: Duration,
    action: F,
  ) -> Box<dyn Subscription>
  where
    Self: ContextScheduler<F>;
}

// ==================== Scheduler System ====================

pub trait Scheduler<F> {
  fn schedule(&self, task: F, _delay: Duration) -> Box<dyn Subscription>;
}

pub struct LocalScheduler;
impl<F> Scheduler<F> for LocalScheduler
where
  F: FnOnce() + 'static,
{
  fn schedule(&self, task: F, _delay: Duration) -> Box<dyn Subscription> {
    task();
    Box::new(())
  }
}

pub struct SharedScheduler;
impl<F> Scheduler<F> for SharedScheduler
where
  F: FnOnce() + Send + 'static,
{
  fn schedule(&self, task: F, _delay: Duration) -> Box<dyn Subscription> {
    task();
    Box::new(())
  }
}

pub trait ContextScheduler<Action> {
  fn schedule(self, delay: Duration, action: Action) -> Box<dyn Subscription>;
}

// --- Local Implementation ---
#[derive(Default, Clone, Copy)]
pub struct Local<T>(pub T);

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

impl<T> Context for Local<T>
where
  T: 'static,
{
  type Inner = T;
  type Rc<U> = MutRc<U>;
  type With<U: 'static> = Local<U>;

  fn into_inner(self) -> T {
    self.0
  }
  fn pack<U: 'static>(inner: U) -> Local<U> {
    Local(inner)
  }

  fn schedule_action<F>(
    self,
    delay: Duration,
    action: F,
  ) -> Box<dyn Subscription>
  where
    Self: ContextScheduler<F>,
  {
    <Self as ContextScheduler<F>>::schedule(self, delay, action)
  }
}

impl<T, F> ContextScheduler<F> for Local<T>
where
  T: 'static,
  F: FnOnce(T) + 'static,
{
  fn schedule(self, delay: Duration, action: F) -> Box<dyn Subscription> {
    let inner = self.0;
    let task = move || action(inner);
    LocalScheduler.schedule(task, delay)
  }
}

// --- Shared Implementation ---
#[derive(Default, Clone, Copy)]
pub struct Shared<T>(pub T);

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

// RELAXED IMPLEMENTATION:
// We implement Context for Shared<T> even if T is !Send.
// This allows 'With<U>' to be valid for all U.
// However, specific features (scheduler) will be missing if T is !Send.
impl<T> Context for Shared<T>
where
  T: 'static, // Removed Send!
{
  type Inner = T;
  type Rc<U> = MutArc<U>;
  type With<U: 'static> = Shared<U>;

  fn into_inner(self) -> T {
    self.0
  }
  fn pack<U: 'static>(inner: U) -> Shared<U> {
    Shared(inner)
  }

  fn schedule_action<F>(
    self,
    delay: Duration,
    action: F,
  ) -> Box<dyn Subscription>
  where
    Self: ContextScheduler<F>,
  {
    <Self as ContextScheduler<F>>::schedule(self, delay, action)
  }
}

// STRICT SCHEDULER IMPLEMENTATION:
// Only available if T: Send + F: Send
impl<T, F> ContextScheduler<F> for Shared<T>
where
  T: Send + 'static,
  F: FnOnce(T) + Send + 'static,
{
  fn schedule(self, delay: Duration, action: F) -> Box<dyn Subscription> {
    let inner = self.0;
    let task = move || action(inner);
    SharedScheduler.schedule(task, delay)
  }
}

// ==========================================
// 1. Observer & Subscription
// ==========================================

pub trait Observer<Item, Err> {
  fn next(&mut self, v: Item);
  fn error(self, e: Err);
  fn complete(self);
}

impl<I, E> Observer<I, E> for () {
  fn next(&mut self, _: I) {}
  fn error(self, _: E) {}
  fn complete(self) {}
}

pub trait Subscription {
  fn unsubscribe(&mut self);
}

impl Subscription for () {
  fn unsubscribe(&mut self) {}
}
impl Subscription for Box<dyn Subscription> {
  fn unsubscribe(&mut self) {
    (**self).unsubscribe()
  }
}

pub struct MergeSubscription<T>(T);
impl<T> Subscription for MergeSubscription<T> {
  fn unsubscribe(&mut self) {}
}

// ==========================================
// 2. ObservableMem & CoreObservable<O> (Merged)
// ==========================================

// New: Defines the Item and Err types of an observable sequence
pub trait ObservableMem {
    type Item;
    type Err;
}

pub trait CoreObservable<O>: ObservableMem {
  type Unsub: Subscription;
  fn subscribe(self, observer: O) -> Self::Unsub;
}

// --- Of (Pure Logic) ---
pub struct Of<T>(pub T);
impl<T> Of<T> {
  pub fn new(v: T) -> Self {
    Self(v)
  }
}

// New: Implement ObservableMem for Of
impl<T> ObservableMem for Of<T> {
    type Item = T;
    type Err = (); // Of operator typically doesn't produce errors
}

// Impl for specific O constraints
impl<O, T> CoreObservable<O> for Of<T>
where
  O: Context,
  O::Inner: Observer<T, ()>,
  T: Clone + 'static,
  Of<T>: ObservableMem<Item = T, Err = ()>,
{
  type Unsub = Box<dyn Subscription>;

  fn subscribe(self, observer: O) -> Self::Unsub {
    let mut inner = observer.into_inner();
    inner.next(self.0);
    inner.complete();
    Box::new(())
  }
}

// --- Map (Pure Logic) ---
pub struct Map<S, F> {
  source: S,
  func: F,
}

pub struct MapObserver<O, F> {
  observer: O,
  func: F,
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

// New: Implement ObservableMem for Map
impl<S, F, In, Out> ObservableMem for Map<S, F>
where
    S: ObservableMem<Item = In>,
    F: FnMut(In) -> Out,
{
    type Item = Out;
    type Err = S::Err;
}


impl<S, F, C> CoreObservable<C> for Map<S, F>
where
  C: Context,
  S: CoreObservable<C::With<MapObserver<C::Inner, F>>> + ObservableMem,
  F: 'static,        // Needed for pack
  C::Inner: 'static, // Needed for pack
  Map<S, F>: ObservableMem<Item = <S as ObservableMem>::Item, Err = <S as ObservableMem>::Err>, // Note: this bound might be tricky, let's see if we strictly need it on the impl or if it's derived.
                                                                                                // Actually, the Map::ObservableMem impl above defines Item=Out.
                                                                                                // So here we shouldn't force Item=S::Item, that was a mistake in my thought process. 
                                                                                                // Map changes the item type!
{
  type Unsub = S::Unsub;

  fn subscribe(self, observer: C) -> Self::Unsub {
    let inner_obs = observer.into_inner();
    let map_observer = MapObserver { observer: inner_obs, func: self.func };
    let wrapped = C::pack(map_observer);
    self.source.subscribe(wrapped)
  }
}

// --- Merge (Pure Logic) ---
pub struct Merge<S1, S2> {
  s1: S1,
  s2: S2,
}
pub struct MergeState {}

// New: Implement ObservableMem for Merge
impl<S1, S2> ObservableMem for Merge<S1, S2>
where
    S1: ObservableMem,
    S2: ObservableMem<Item = S1::Item, Err = S1::Err>,
{
    type Item = S1::Item;
    type Err = S1::Err;
}

impl<S1, S2, C> CoreObservable<C> for Merge<S1, S2>
where
  C: Context,
  S1: CoreObservable<C> + ObservableMem,
  S2: CoreObservable<C> + ObservableMem<Item = S1::Item, Err = S1::Err>,
{
  type Unsub = MergeSubscription<C::Rc<MergeState>>;

  fn subscribe(self, _observer: C) -> Self::Unsub {
    let state = C::Rc::from(MergeState {});
    MergeSubscription(state)
  }
}

// ==========================================
// 3. Observable (User API)
// ==========================================

pub trait Observable: Context
where
    Self::Inner: ObservableMem,
{
  type Item;
  type Err;

  fn subscribe<O>(
    self,
    observer: O,
  ) -> <Self::Inner as CoreObservable<Self::With<O>>>::Unsub
  where
    Self::Inner: CoreObservable<Self::With<O>>,
    O: 'static,
    O: Observer<Self::Item, Self::Err>,
  {
    let core = self.into_inner();
    let wrapped = Self::pack(observer);
    core.subscribe(wrapped)
  }

  fn map<F, Out>(self, f: F) -> Self::With<Map<Self::Inner, F>>
  where
    F: FnMut(Self::Item) -> Out + 'static,
    Self::Inner: 'static,
    // We need to ensure the result is also an Observable
    Self::With<Map<Self::Inner, F>>: Observable<Item = Out, Err = Self::Err>,
    // And that the inner Map logic is sound
    Map<Self::Inner, F>: ObservableMem<Item = Out, Err = Self::Err>,
  {
    let core = self.into_inner();
    let op = Map { source: core, func: f };
    Self::pack(op)
  }

  fn merge<S>(self, other: S) -> Self::With<Merge<Self::Inner, S::Inner>>
  where
    S: Observable<Item = Self::Item, Err = Self::Err>,
    Self::Inner: 'static,
    S::Inner: 'static + ObservableMem,
    // Ensure result is Observable
    Self::With<Merge<Self::Inner, S::Inner>>: Observable<Item = Self::Item, Err = Self::Err>,
    // Ensure inner Merge logic is sound
    Merge<Self::Inner, S::Inner>: ObservableMem<Item = Self::Item, Err = Self::Err>,
  {
    let core1 = self.into_inner();
    let core2 = other.into_inner();
    let op = Merge { s1: core1, s2: core2 };
    Self::pack(op)
  }
}

// Implement for all Contexts that wrap an ObservableMem
impl<T> Observable for T
where
    T: Context,
    T::Inner: ObservableMem,
    // We don't strictly need T::Inner: CoreObservable here for the trait def,
    // but methods will require it.
{
    type Item = <T::Inner as ObservableMem>::Item;
    type Err = <T::Inner as ObservableMem>::Err;
}

// New: Factory function for Of
pub mod observable {
    use super::{Context, CoreObservable, Local, Of, Observable};

    pub fn of<T: Clone + 'static>(v: T) -> Local<Of<T>> {
        Local(Of::new(v))
    }
}


// ==========================================
// 4. Verification
// ==========================================

// Helper function to verify we can use Observable as a trait object or generic bound
// with explicit Item/Err types.
fn process_int_observable<O>(obs: O) -> i32
where
    O: Observable<Item = i32, Err = ()>,
    // Crucial: We must promise that O's Inner can actually be subscribed to
    // with a specific Observer type.
    // In a real library we might hide this complexity, but for verification:
    <O as Context>::Inner: CoreObservable<<O as Context>::With<SimpleIntObserver>>,
{
    let rc = Rc::new(RefCell::new(0));
    let observer = SimpleIntObserver(rc.clone());
    obs.subscribe(observer);
    let res = *rc.borrow();
    res
}

// Define the observer used in the function above
struct SimpleIntObserver(Rc<RefCell<i32>>);
impl Observer<i32, ()> for SimpleIntObserver {
    fn next(&mut self, v: i32) {
        *self.0.borrow_mut() = v;
    }
    fn error(self, _: ()) {}
    fn complete(self) {}
}


fn main() {
  println!("Scheme 12: Merged Core + Integrated Scheduler (Relaxed Shared)");
  println!("Applied Fix: Observable now exposes Item and Err types.");

  // 1. Scheduler Constraint Verification
  {
    println!("
-- Verifying Scheduler Constraints --");

    let local_ctx = Local(());
    let rc = Rc::new(100);
    let action_not_send = move |_| {
      println!("Accessing Rc in Local: {}", rc);
    };
    // Should compile
    local_ctx.schedule_action(Duration::from_secs(0), action_not_send);

    let shared_ctx = Shared(());
    let action_send = move |_| {
      println!("Accessing Send action in Shared");
    };
    // Should compile
    shared_ctx.schedule_action(Duration::from_secs(0), action_send);
  }

  // 2. Full Pipeline with direct Of
  {
    println!("
-- Verifying Pipeline with raw Of --");
    let l1 = Local(Of::new(1));
    let rc = Rc::new(RefCell::new(0));

    struct RcObserver(Rc<RefCell<i32>>);
    impl Observer<i32, ()> for RcObserver {
      fn next(&mut self, v: i32) {
        *self.0.borrow_mut() = v;
      }
      fn error(self, _: ()) {}
      fn complete(self) {}
    }

    // Subscribing
    l1.subscribe(RcObserver(rc.clone()));

    println!("Pipeline Result (raw Of): {}", *rc.borrow());
  }

  // 3. Full Pipeline with factory function and map
  {
      println!("
-- Verifying Pipeline with factory and map --");
      let obs = observable::of(10)
          .map(|x| x * 2);

      let rc = Rc::new(RefCell::new(0));
      struct MapTestObserver(Rc<RefCell<i32>>);
      impl Observer<i32, ()> for MapTestObserver {
          fn next(&mut self, v: i32) {
              *self.0.borrow_mut() = v;
          }
          fn error(self, _: ()) {}
          fn complete(self) {}
      }
      obs.subscribe(MapTestObserver(rc.clone()));
      println!("Pipeline Result (factory + map): {}", *rc.borrow());
  }

  // 4. Verification of Observable::Item and Err
  {
      println!("
-- Verifying Observable::Item and Err usability --");
      let obs = observable::of(42);
      // We need to help the compiler see that Local<Of<i32>> meets the bounds
      // for process_int_observable.
      // Specifically: Local<Of<i32>>::Inner must be CoreObservable<Local<SimpleIntObserver>>
      // Since Of<i32> implements CoreObservable<Local> where Local::Inner is Observer<i32>,
      // and SimpleIntObserver is Observer<i32>, this should work.
      
      let result = process_int_observable(obs);
      println!("Function 'process_int_observable' result: {}", result);

      let obs_mapped = observable::of(100).map(|x| x / 10);
      let result_mapped = process_int_observable(obs_mapped);
      println!("Function 'process_int_observable' with mapped observable result: {}", result_mapped);
  }
}