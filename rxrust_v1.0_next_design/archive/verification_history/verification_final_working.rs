use std::marker::PhantomData;
use std::rc::Rc;

// ==================== Basic Infrastructure ====================

pub trait Subscription {
  fn unsubscribe(self);
}
impl Subscription for () {
  fn unsubscribe(self) {}
}

pub trait Observer<Item, Err> {
  fn next(&mut self, value: Item);
  fn error(self, err: Err);
  fn complete(self);
}

pub trait Context: Sized {
  type Inner;
  fn into_inner(self) -> Self::Inner;
}

pub struct Local<T>(pub T);
impl<T> Context for Local<T> {
  type Inner = T;
  fn into_inner(self) -> T {
    self.0
  }
}

pub struct Shared<T>(pub T);
impl<T> Context for Shared<T> {
  type Inner = T;
  fn into_inner(self) -> T {
    self.0
  }
}

// ==================== Scheduler System ====================

pub trait Runnable {
  fn run(self);
}

// Task structs with PhantomData to constrain all generics
pub struct OnNext<O, Item, Err> {
  pub observer: O,
  pub item: Item,
  pub _p: PhantomData<Err>,
}
impl<O, Item, Err> Runnable for OnNext<O, Item, Err>
where
  O: Observer<Item, Err>,
{
  fn run(mut self) {
    self.observer.next(self.item);
  }
}

pub struct OnError<O, Item, Err> {
  pub observer: O,
  pub err: Err,
  pub _p: PhantomData<Item>,
}
impl<O, Item, Err> Runnable for OnError<O, Item, Err>
where
  O: Observer<Item, Err>,
{
  fn run(self) {
    self.observer.error(self.err);
  }
}

pub struct OnComplete<O, Item, Err> {
  pub observer: O,
  pub _p: PhantomData<(Item, Err)>,
}
impl<O, Item, Err> Runnable for OnComplete<O, Item, Err>
where
  O: Observer<Item, Err>,
{
  fn run(self) {
    self.observer.complete();
  }
}

// The Scheduler Trait
pub trait Scheduler<F> {
  fn schedule(&self, task: F);
}

#[derive(Clone)]
pub struct LocalScheduler;
impl<F> Scheduler<F> for LocalScheduler
where
  F: Runnable,
{
  fn schedule(&self, task: F) {
    task.run();
  }
}

#[derive(Clone)]
pub struct SharedScheduler;
impl<F> Scheduler<F> for SharedScheduler
where
  F: Runnable + Send,
{
  fn schedule(&self, task: F) {
    let _ = task;
  }
}

// ==================== CoreSubscriberGuard ====================

pub trait CoreSubscriberGuard<O> {
  // Renamed to avoid ambiguity with ObservableCore::Unsub
  type GuardUnsub: Subscription;

  fn handle_observer(self, observer: O) -> Self::GuardUnsub;
}

// ==================== ObservableCore ====================

pub trait ObservableCore: Sized {
  type Item;
  type Err;

  // GAT dependent on Context, delegated to Guard
  type Unsub<C: Context>: Subscription
  where
    Self: CoreSubscriberGuard<C::Inner>;

  fn subscribe_core<C>(self, observer: C) -> Self::Unsub<C>
  where
    C: Context,
    C::Inner: Observer<Self::Item, Self::Err>,
    Self: CoreSubscriberGuard<C::Inner>;
}

// ==================== Implementation: ObserveOn ====================

pub struct ObserveOn<S, Sched> {
  pub source: S,
  pub scheduler: Sched,
}

pub struct ObserveOnObserver<O, Sched, Item, Err> {
  pub observer: O,
  pub scheduler: Sched,
  pub _p: PhantomData<(Item, Err)>,
}

// 1. Implement Observer for the Wrapper
impl<O, Sched, Item, Err> Observer<Item, Err>
  for ObserveOnObserver<O, Sched, Item, Err>
where
  O: Observer<Item, Err> + Clone,
  // The Scheduler Constraints!
  Sched: Scheduler<OnNext<O, Item, Err>>
    + Scheduler<OnError<O, Item, Err>>
    + Scheduler<OnComplete<O, Item, Err>>,
{
  fn next(&mut self, value: Item) {
    let task = OnNext {
      observer: self.observer.clone(),
      item: value,
      _p: PhantomData,
    };
    self.scheduler.schedule(task);
  }
  fn error(self, err: Err) {
    let task = OnError {
      observer: self.observer.clone(),
      err,
      _p: PhantomData,
    };
    self.scheduler.schedule(task);
  }
  fn complete(self) {
    let task = OnComplete {
      observer: self.observer.clone(),
      _p: PhantomData,
    };
    self.scheduler.schedule(task);
  }
}

// 2. Implement CoreSubscriberGuard for ObserveOn
impl<S, Sched, O, Item, Err> CoreSubscriberGuard<O> for ObserveOn<S, Sched>
where
  S: ObservableCore<Item = Item, Err = Err>,
  Sched: Clone,
  // Requirement A: Wrapper must be a valid Observer (implies Sched check)
  ObserveOnObserver<O, Sched, Item, Err>: Observer<Item, Err>,
  // Requirement B: Source must accept wrapper
  S: CoreSubscriberGuard<ObserveOnObserver<O, Sched, Item, Err>>,
{
  type GuardUnsub = <S as CoreSubscriberGuard<
    ObserveOnObserver<O, Sched, Item, Err>,
  >>::GuardUnsub;

  fn handle_observer(self, observer: O) -> Self::GuardUnsub {
    let wrapped = ObserveOnObserver {
      observer,
      scheduler: self.scheduler,
      _p: PhantomData,
    };
    self.source.handle_observer(wrapped)
  }
}

// 3. Implement ObservableCore for ObserveOn
impl<S, Sched, Item, Err> ObservableCore for ObserveOn<S, Sched>
where
  S: ObservableCore<Item = Item, Err = Err>,
  Sched: Clone,
{
  type Item = Item;
  type Err = Err;

  // Map Unsub to GuardUnsub
  type Unsub<C: Context>
    = <Self as CoreSubscriberGuard<C::Inner>>::GuardUnsub
  where
    Self: CoreSubscriberGuard<C::Inner>;

  fn subscribe_core<C>(self, observer: C) -> Self::Unsub<C>
  where
    C: Context,
    C::Inner: Observer<Self::Item, Self::Err>,
    Self: CoreSubscriberGuard<C::Inner>,
  {
    self.handle_observer(observer.into_inner())
  }
}

// ==================== Mock Source ====================

pub struct MockSource;
impl ObservableCore for MockSource {
  type Item = i32;
  type Err = ();

  // Link Unsub directly to GuardUnsub
  type Unsub<C: Context>
    = <Self as CoreSubscriberGuard<C::Inner>>::GuardUnsub
  where
    Self: CoreSubscriberGuard<C::Inner>;

  fn subscribe_core<C>(self, observer: C) -> Self::Unsub<C>
  where
    C: Context,
    C::Inner: Observer<i32, ()>,
    Self: CoreSubscriberGuard<C::Inner>,
  {
    self.handle_observer(observer.into_inner())
  }
}

impl<O> CoreSubscriberGuard<O> for MockSource
where
  O: Observer<i32, ()>,
{
  type GuardUnsub = ();
  fn handle_observer(self, _observer: O) -> () {}
}

// ==================== Validation ====================

#[derive(Clone)]
struct RcObserver(Rc<i32>); // !Send
impl Observer<i32, ()> for RcObserver {
  fn next(&mut self, _: i32) {}
  fn error(self, _: ()) {}
  fn complete(self) {}
}

#[derive(Clone)]
struct SendObserver; // Send
impl Observer<i32, ()> for SendObserver {
  fn next(&mut self, _: i32) {}
  fn error(self, _: ()) {}
  fn complete(self) {}
}

fn main() {
  // 1. Local + Local (Compatible)
  {
    let op = ObserveOn {
      source: MockSource,
      scheduler: LocalScheduler,
    };
    let ctx = Local(RcObserver(Rc::new(1)));
    op.subscribe_core(ctx);
  }

  // 2. Shared + Shared (Compatible)
  {
    let op = ObserveOn {
      source: MockSource,
      scheduler: SharedScheduler,
    };
    let ctx = Shared(SendObserver);
    op.subscribe_core(ctx);
  }

  // 3. Shared + Local Scheduler (Compatible)
  {
    let op = ObserveOn {
      source: MockSource,
      scheduler: LocalScheduler,
    };
    let ctx = Shared(SendObserver);
    op.subscribe_core(ctx);
  }

  // 4. Local + Shared (INCOMPATIBLE)
  {
    let op = ObserveOn {
      source: MockSource,
      scheduler: SharedScheduler,
    };
    let ctx = Local(RcObserver(Rc::new(1)));

    // Uncomment to verify failure:
    // error[E0277]: the trait bound `SharedScheduler: Scheduler<OnNext<RcObserver, i32, ()>>` is not satisfied
    // op.subscribe_core(ctx);
  }
}
