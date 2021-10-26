use crate::prelude::*;
use crate::{complete_proxy_impl, error_proxy_impl};

#[derive(Clone)]
pub struct ScanOp<Source, BinaryOp, OutputItem> {
  pub(crate) source_observable: Source,
  pub(crate) binary_op: BinaryOp,
  pub(crate) initial_value: OutputItem,
}

pub struct ScanObserver<Observer, BinaryOp, OutputItem, InputItem> {
  target_observer: Observer,
  binary_op: BinaryOp,
  acc: OutputItem,
  _marker: TypeHint<InputItem>,
}

#[doc(hidden)]
macro_rules! observable_impl {
    ($subscription:ty, $($marker:ident +)* $lf: lifetime) => {
  fn actual_subscribe<O>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub
  where O: Observer<Item=Self::Item,Err= Self::Err> + $($marker +)* $lf {
    self.source_observable.actual_subscribe(Subscriber {
      observer: ScanObserver {
        target_observer: subscriber.observer,
        binary_op: self.binary_op,
        acc: self.initial_value,
        _marker: TypeHint::new(),
      },
      subscription: subscriber.subscription,
    })
  }
}
}

impl<OutputItem, Source, BinaryOp> Observable
  for ScanOp<Source, BinaryOp, OutputItem>
where
  Source: Observable,
  OutputItem: Clone,
  BinaryOp: FnMut(OutputItem, Source::Item) -> OutputItem,
{
  type Item = OutputItem;
  type Err = Source::Err;
}

// We're making the `ScanOp` being an publisher - an object that
// subscribers can subscribe to.
// Doing so by implementing `Observable` trait for it.
// Once instantiated, it will have a `actual_subscribe` method in it.
// Note: we're accepting such subscribers that accept `Output` as their
// `Item` type.
impl<'a, OutputItem, Source, BinaryOp> LocalObservable<'a>
  for ScanOp<Source, BinaryOp, OutputItem>
where
  Source: LocalObservable<'a>,
  OutputItem: Clone + 'a,
  BinaryOp: FnMut(OutputItem, Source::Item) -> OutputItem + 'a,
  Source::Item: 'a,
{
  type Unsub = Source::Unsub;
  observable_impl!(LocalSubscription, 'a);
}

impl<OutputItem, Source, BinaryOp> SharedObservable
  for ScanOp<Source, BinaryOp, OutputItem>
where
  Source: SharedObservable,
  OutputItem: Clone + Send + Sync + 'static,
  Source::Item: 'static,
  BinaryOp:
    FnMut(OutputItem, Source::Item) -> OutputItem + Send + Sync + 'static,
{
  type Unsub = Source::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

// We're making `ScanObserver` being able to be subscribed to other observables
// by implementing `Observer` trait. Thanks to this, it is able to observe
// sources having `Item` type as its `InputItem` type.
impl<InputItem, Err, Source, BinaryOp, OutputItem> Observer
  for ScanObserver<Source, BinaryOp, OutputItem, InputItem>
where
  Source: Observer<Item = OutputItem, Err = Err>,
  BinaryOp: FnMut(OutputItem, InputItem) -> OutputItem,
  OutputItem: Clone,
{
  type Item = InputItem;
  type Err = Err;
  fn next(&mut self, value: InputItem) {
    // accumulating each item with a current value
    self.acc = (self.binary_op)(self.acc.clone(), value);
    self.target_observer.next(self.acc.clone())
  }

  error_proxy_impl!(Err, target_observer);
  complete_proxy_impl!(target_observer);
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn scan_initial() {
    let mut emitted = Vec::<i32>::new();
    // should work like accumulate from 100
    observable::from_iter(vec![1, 1, 1, 1, 1])
      .scan_initial(100, |acc, v| acc + v)
      .subscribe(|v| emitted.push(v));

    assert_eq!(vec!(101, 102, 103, 104, 105), emitted);
  }
  #[test]
  fn scan_initial_on_empty_observable() {
    let mut emitted = Vec::<i32>::new();
    // should work like accumulate from 100
    observable::empty()
      .scan_initial(100, |acc, v: i32| acc + v)
      .subscribe(|v| emitted.push(v));

    assert_eq!(Vec::<i32>::new(), emitted);
  }

  #[test]
  fn scan_initial_mixed_types() {
    let mut emitted = Vec::<i32>::new();
    // Should work like accumulate from 100,
    // as we ignore the input characters and just
    // increment the accumulated value given.
    observable::from_iter(vec!['a', 'b', 'c', 'd', 'e'])
      .scan_initial(100, |acc, _v| acc + 1)
      .subscribe(|v| emitted.push(v));

    assert_eq!(vec!(101, 102, 103, 104, 105), emitted);
  }

  #[test]
  fn scan_with_default() {
    let mut emitted = Vec::<i32>::new();
    // should work like accumulate from 0
    observable::from_iter(vec![1, 1, 1, 1, 1])
      .scan(|acc, v| acc + v)
      .subscribe(|v| emitted.push(v));

    assert_eq!(vec!(1, 2, 3, 4, 5), emitted);
  }

  #[test]
  fn scan_fork_and_shared_mixed_types() {
    // type to type can fork
    let m = observable::from_iter(vec!['a', 'b', 'c']).scan(|_acc, _v| 1i32);
    m.scan(|_acc, v| v as f32)
      .into_shared()
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn scan_fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(0..100).scan(|acc: i32, v| acc + v);
    m.scan(|acc: i32, v| acc + v)
      .into_shared()
      .into_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_scan);

  fn bench_scan(b: &mut bencher::Bencher) { b.iter(scan_initial); }
}
