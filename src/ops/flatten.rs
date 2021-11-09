use crate::{impl_helper::*, impl_local_shared_both, prelude::*};
////////////////////////////////////////////////////////////////////////////////

#[derive(Clone)]
/// Operator to merge Observables
pub struct FlattenOp<S, Inner> {
  pub(crate) source: S,
  pub(crate) marker: std::marker::PhantomData<Inner>,
}

impl<Outer, Inner> Observable for FlattenOp<Outer, Inner>
where
  Inner: Observable,
  Outer: Observable<Item = Inner, Err = Inner::Err>,
{
  type Item = Inner::Item;
  type Err = Inner::Err;
}

////////////////////////////////////////////////////////////////////////////////

/// Keeps track of how many observables are being observed at any point in time.
///
/// Because we are subscribed to an Observable of Observables we need to keep
/// track of every new Observable that is emitted from the source Observable.
pub struct FlattenState {
  total: u64,
  done: u64,
  is_completed: bool,
}

impl FlattenState {
  /// Creates a new state for a Flatten operator.
  #[inline]
  pub fn new() -> Self { Self::default() }

  /// Indicates if a completion of emissions has been detected. This happens
  /// when the number of new Observables is the same as the number of
  /// completed Observables.
  pub fn is_completed(&self) -> bool { self.is_completed }

  /// Records the registration of a new Observable.
  pub fn register_new_observable(&mut self) {
    if self.is_completed {
      return;
    }

    self.total += 1;
  }

  /// Records the signaling of an error from any registered Observable.
  pub fn register_observable_error(&mut self) -> bool {
    if self.is_completed {
      // signal not to register error on observer, as it was completed already
      false
    } else {
      // ensure to complete the state machine and signal the observer should
      // receive an error call
      self.is_completed = true;
      true
    }
  }

  /// Records the signaling of completion from any registered Observable.
  pub fn register_observable_completed(&mut self) -> bool {
    if self.is_completed {
      // return signal to not complete observer, as it has been already
      // completed
      return false;
    }

    self.done += 1;

    if self.total == self.done {
      self.is_completed = true;
      // report signal to complete observer
      true
    } else {
      // report signal to not complete observer
      false
    }
  }
}

impl Default for FlattenState {
  fn default() -> Self {
    FlattenState {
      // when this record is created, we are subscribing to an observable of
      // observables, so it must be accounted for from the get-go
      total: 1,
      done: 0,
      is_completed: false,
    }
  }
}

////////////////////////////////////////////////////////////////////////////////
// Inner observer

#[derive(Clone)]
/// This is an `Observer` for items of an `Observable` that is emitted from a
/// parent `Observable`.
pub struct FlattenInnerObserver<O, U, S> {
  observer: O,
  subscription: U,
  state: S,
}

impl<O, U, S> Observer for FlattenInnerObserver<O, U, S>
where
  O: Observer,
  U: SubscriptionLike,
  S: RcDerefMut,
  for<'r> S::Target<'r>: std::ops::DerefMut<Target = FlattenState>,
{
  type Item = O::Item;
  type Err = O::Err;

  fn next(&mut self, item: Self::Item) {
    if !self.state.rc_deref_mut().is_completed {
      self.observer.next(item);
    }
  }

  fn error(&mut self, err: Self::Err) {
    if self.state.rc_deref_mut().register_observable_error() {
      self.observer.error(err);
      self.subscription.unsubscribe();
    }
  }

  fn complete(&mut self) {
    if self.state.rc_deref_mut().register_observable_completed() {
      self.observer.complete();
      self.subscription.unsubscribe();
    }
  }
}

#[derive(Clone)]
/// This is an `Observer` for `Observable` values that get emitted by an
/// `Observable` that works on a shared environment.
pub struct FlattenOuterObserver<Inner, InnerObserver, U, S> {
  marker: std::marker::PhantomData<Inner>,
  inner_observer: InnerObserver,
  subscription: U,
  state: S,
}

type FlattenSharedOuterObserver<Inner, InnerObserver> = FlattenOuterObserver<
  Inner,
  MutArc<
    FlattenInnerObserver<
      InnerObserver,
      SharedSubscription,
      MutArc<FlattenState>,
    >,
  >,
  SharedSubscription,
  MutArc<FlattenState>,
>;

type FlattenLocalOuterObserver<Inner, InnerObserver> = FlattenOuterObserver<
  Inner,
  MutRc<
    FlattenInnerObserver<InnerObserver, LocalSubscription, MutRc<FlattenState>>,
  >,
  LocalSubscription,
  MutRc<FlattenState>,
>;

macro_rules! impl_outer_obsrever {
  () => {
    type Item = Inner;
    type Err = O::Err;

    fn next(&mut self, value: Inner) {
      // increase count of registered Observables to keep track
      // of observable completion
      self.state.rc_deref_mut().register_new_observable();

      self
        .subscription
        .add(value.actual_subscribe(self.inner_observer.clone()));
    }

    fn error(&mut self, err: Self::Err) { self.inner_observer.error(err) }

    fn complete(&mut self) { self.inner_observer.complete() }
  };
}

impl<Inner, O> Observer for FlattenSharedOuterObserver<Inner, O>
where
  O: Observer + Sync + Send + 'static,
  Inner: SharedObservable<Item = O::Item, Err = O::Err>,
{
  impl_outer_obsrever!();
}

impl<'a, Inner, O, Item, Err> Observer for FlattenLocalOuterObserver<Inner, O>
where
  O: Observer<Item = Item, Err = Err> + 'a,
  Inner: LocalObservable<'a, Item = Item, Err = Err>,
  Inner::Unsub: 'static,
{
  impl_outer_obsrever!();
}

impl_local_shared_both! {
  impl<Outer, Inner> FlattenOp<Outer, Inner>;
  type Unsub = @ctx::RcMultiSubscription;
  macro method($self: ident, $observer:ident, $ctx: ident) {
    let state = $ctx::Rc::own(FlattenState::new());

    let subscription = $ctx::RcMultiSubscription::default();
    let inner_observer = $ctx::Rc::own(FlattenInnerObserver {
      observer: $observer,
      subscription: subscription.clone(),
      state: state.clone(),
    });

    let observer = FlattenOuterObserver {
      marker: std::marker::PhantomData::<Inner>,
      inner_observer,
      subscription: subscription.clone(),
      state,
    };

    subscription.add($self.source.actual_subscribe(observer));

    subscription
  }
  where
    Inner: @ctx::Observable
      @ctx::shared_only(+ Send + Sync + 'static)
      @ctx::local_only(+ 'o),
    Outer: @ctx::Observable<Item=Inner, Err=Inner::Err>,
    Inner::Unsub: 'static,
    Outer::Unsub: 'static
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
  };

  #[test]
  fn odd_even_flatten() {
    let mut odd_store = vec![];
    let mut even_store = vec![];
    let mut numbers_store = vec![];

    {
      let mut sources = LocalSubject::new();

      let numbers = sources.clone().flatten();
      let odd = numbers.clone().filter(|v: &i32| *v % 2 != 0);
      let even = numbers.clone().filter(|v: &i32| *v % 2 == 0);

      numbers.subscribe(|v: i32| numbers_store.push(v));
      odd.subscribe(|v: i32| odd_store.push(v));
      even.subscribe(|v: i32| even_store.push(v));

      (0..10).for_each(|v| {
        let source = observable::of(v);
        sources.next(source);
      });
    }

    assert_eq!(even_store, vec![0, 2, 4, 6, 8]);
    assert_eq!(odd_store, vec![1, 3, 5, 7, 9]);
    assert_eq!(numbers_store, (0..10).collect::<Vec<_>>());
  }

  #[test]
  fn flatten_unsubscribe_work() {
    let mut source = LocalSubject::new();

    let sources = source.clone().map(|v| observable::from_iter(vec![v]));
    let numbers = sources.flatten();
    // enabling multiple observers for even stream;
    let _even = numbers.clone().filter(|v| *v % 2 == 0);
    // enabling multiple observers for odd stream;
    let _odd = numbers.clone().filter(|v| *v % 2 != 0);

    numbers
      .subscribe(|_| unreachable!("oh, unsubscribe does not work."))
      .unsubscribe();

    source.next(&1);
  }

  #[test]
  fn flatten_completed_test() {
    let completed = Arc::new(AtomicBool::new(false));
    let c_clone = completed.clone();

    let mut source = LocalSubject::new();
    let mut one = LocalSubject::new();
    let mut two = LocalSubject::new();

    let out = source.clone().flatten();

    // we need to subscribe to out first to keep track of the
    // events from source
    out.subscribe_complete(
      |_: &()| {},
      move || {
        println!("subscribe_complete complete callback done");
        completed.store(true, Ordering::Relaxed);
      },
    );

    source.next(one.clone());
    source.next(two.clone());

    one.complete();
    assert!(!c_clone.load(Ordering::Relaxed));

    two.complete();
    assert!(!c_clone.load(Ordering::Relaxed));

    source.complete();
    assert!(c_clone.load(Ordering::Relaxed));
  }

  #[test]
  fn flatten_error_test() {
    let completed = Arc::new(Mutex::new(0));
    let cc = completed.clone();

    let error = Arc::new(Mutex::new(0));
    let ec = error.clone();

    let mut source = LocalSubject::new();
    let mut even = LocalSubject::new();
    let mut odd = LocalSubject::new();

    let output = source.clone().flatten();

    output.subscribe_all(
      |_: ()| {},
      move |_| *error.lock().unwrap() += 1,
      move || *completed.lock().unwrap() += 1,
    );

    source.next(even.clone());
    source.next(odd.clone());

    odd.error("");
    even.error("");
    even.complete();

    // if error occur, stream terminated.
    assert_eq!(*cc.lock().unwrap(), 0);
    // error should be hit just once
    assert_eq!(*ec.lock().unwrap(), 1);
  }

  #[test]
  fn flatten_local_and_shared() {
    let mut res = vec![];

    let mut source = SharedSubject::new();
    let local1 = observable::of(1);
    let local2 = observable::of(2);

    let shared = source.clone().flatten().into_shared();

    shared.subscribe(move |v: i32| {
      res.push(v);
    });

    source.next(local1);
    source.next(local2);
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_flatten);

  fn bench_flatten(b: &mut bencher::Bencher) { b.iter(odd_even_flatten); }
}
