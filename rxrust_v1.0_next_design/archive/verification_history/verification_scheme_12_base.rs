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
// 2. CoreObservable<O> (Merged)
// ==========================================

pub trait CoreObservable<O> {
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

// Impl for specific O constraints
impl<O, T> CoreObservable<O> for Of<T>
where
  O: Context,
  O::Inner: Observer<T, ()>,
  T: Clone + 'static,
{
  type Unsub = Box<dyn Subscription>;

  fn subscribe(self, observer: O) -> Self::Unsub {
    // We run immediately here to simplify checking 'main' logic constraints.
    // A real implementation would try to use schedule_action.
    // But verifying schedule_action inside generic code is hard without specific bounds.
    // We verified schedule_action works in 'main'.

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

impl<S, F, C> CoreObservable<C> for Map<S, F>
where
  C: Context,
  S: CoreObservable<C::With<MapObserver<C::Inner, F>>>,
  F: 'static,        // Needed for pack
  C::Inner: 'static, // Needed for pack
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

impl<S1, S2, C> CoreObservable<C> for Merge<S1, S2>
where
  C: Context,
  S1: CoreObservable<C>,
  S2: CoreObservable<C>,
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

pub trait Observable: Context {
  fn subscribe<O>(
    self,
    observer: O,
  ) -> <Self::Inner as CoreObservable<Self::With<O>>>::Unsub
  where
    Self::Inner: CoreObservable<Self::With<O>>,
    O: 'static, // Implicit bound for With<O>
  {
    let core = self.into_inner();
    let wrapped = Self::pack(observer);
    core.subscribe(wrapped)
  }

  fn map<F>(self, f: F) -> Self::With<Map<Self::Inner, F>>
  where
    F: 'static,
    Self::Inner: 'static,
  {
    let core = self.into_inner();
    let op = Map { source: core, func: f };
    Self::pack(op)
  }

  fn merge<S>(self, other: S) -> Self::With<Merge<Self::Inner, S::Inner>>
  where
    S: Observable,
    Self::Inner: 'static,
    S::Inner: 'static,
  {
    let core1 = self.into_inner();
    let core2 = other.into_inner();
    let op = Merge { s1: core1, s2: core2 };
    Self::pack(op)
  }
}

// Implement for all Contexts
impl<T: Context> Observable for T {}

// ==========================================
// 4. Verification
// ==========================================

fn main() {
  println!("Scheme 12: Merged Core + Integrated Scheduler (Relaxed Shared)");

  // 1. Scheduler Constraint Verification
  {
    println!("-- Verifying Scheduler Constraints --");

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

    // UNCOMMENT TO FAIL:
    // let shared_ctx2 = Shared(());
    // let rc2 = Rc::new(200);
    // let bad_action = move |_| { println!("{}", rc2); };
    // shared_ctx2.schedule_action(Duration::ZERO, bad_action);
    // ^ Error: Shared<T> implements ContextScheduler<F> only if F: Send
  }

  // 2. Full Pipeline
  {
    println!("-- Verifying Pipeline --");
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

    println!("Pipeline Result: {}", *rc.borrow());
  }
}
