use crate::observer::{complete_proxy_impl, error_proxy_impl};
use crate::prelude::*;

/// The Scan operator applies a function to the first item emitted by the
/// source observable and then emits the result of that function as its
/// own first emission. It also feeds the result of the function back into
/// the function along with the second item emitted by the source observable
/// in order to generate its second emission. It continues to feed back its
/// own subsequent emissions along with the subsequent emissions from the
/// source Observable in order to create the rest of its sequence.
pub trait Scan<OutputItem> {
  /// Applies a binary operator closure to each item emitted from source
  /// observable and emits successive values.
  ///
  /// Completes when source observable completes.
  /// Emits error when source observable emits it.
  ///
  /// This version starts with an user-specified initial value for when the
  /// binary operator is called with the first item processed.
  ///
  /// # Arguments
  ///
  /// * `initial_value` - An initial value to start the successive accumulations
  ///   from.
  /// * `binary_op` - A closure or function acting as a binary operator.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  /// use rxrust::ops::Scan;
  ///
  /// observable::from_iter(vec![1, 1, 1, 1, 1])
  ///   .scan_initial(100, |acc, v| acc + v)
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 101
  /// // 102
  /// // 103
  /// // 104
  /// // 105
  /// ```
  ///
  fn scan_initial<InputItem, BinaryOp>(
    self,
    initial_value: OutputItem,
    binary_op: BinaryOp,
  ) -> ScanOp<Self, BinaryOp, OutputItem>
  where
    Self: Sized,
    BinaryOp: Fn(OutputItem, InputItem) -> OutputItem,
  {
    ScanOp {
      source_observable: self,
      binary_op,
      initial_value,
    }
  }

  /// Works like [`scan_initial`] but starts with a value defined by a
  /// [`Default`] trait for the first argument `binary_op` operator
  /// operates on.
  ///
  /// # Arguments
  ///
  /// * `binary_op` - A closure or function acting as a binary operator.
  ///
  fn scan<InputItem, BinaryOp>(
    self,
    binary_op: BinaryOp,
  ) -> ScanOp<Self, BinaryOp, OutputItem>
  where
    Self: Sized,
    BinaryOp: Fn(OutputItem, InputItem) -> OutputItem,
    OutputItem: Default,
  {
    self.scan_initial(OutputItem::default(), binary_op)
  }
}

impl<O, OutputItem> Scan<OutputItem> for O {}

#[derive(Clone)]
pub struct ScanOp<Source, BinaryOp, OutputItem> {
  source_observable: Source,
  binary_op: BinaryOp,
  initial_value: OutputItem,
}

pub struct ScanObserver<Observer, BinaryOp, OutputItem> {
  target_observer: Observer,
  binary_op: BinaryOp,
  acc: OutputItem,
}

macro observable_impl($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
    self.source_observable.actual_subscribe(Subscriber {
      observer: ScanObserver {
        target_observer: subscriber.observer,
        binary_op: self.binary_op,
        acc: self.initial_value,
      },
      subscription: subscriber.subscription,
    })
  }
}

// We're making the `ScanOp` being an publisher - an object that
// subscribers can subscribe to.
// Doing so by implementing `Observable` trait for it.
// Once instantiated, it will have a `actual_subscribe` method in it.
// Note: we're accepting such subscribers that accept `Output` as their
// `Item` type.
impl<'a, OutputItem, Source, BinaryOp> Observable<'a>
  for ScanOp<Source, BinaryOp, OutputItem>
where
  Source: Observable<'a>,
  OutputItem: Clone + 'a,
  BinaryOp: FnMut(OutputItem, Source::Item) -> OutputItem + 'a,
{
  type Item = OutputItem;
  type Err = Source::Err;
  type Unsub = Source::Unsub;
  observable_impl!(LocalSubscription, 'a);
}

impl<OutputItem, Source, BinaryOp> SharedObservable
  for ScanOp<Source, BinaryOp, OutputItem>
where
  Source: SharedObservable,
  OutputItem: Clone + Send + Sync + 'static,
  BinaryOp:
    FnMut(OutputItem, Source::Item) -> OutputItem + Send + Sync + 'static,
{
  type Item = OutputItem;
  type Err = Source::Err;
  type Unsub = Source::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

// We're making `ScanObserver` being able to be subscribed to other observables
// by implementing `Observer` trait. Thanks to this, it is able to observe
// sources having `Item` type as its `InputItem` type.

impl<InputItem, Err, Source, BinaryOp, OutputItem> Observer<InputItem, Err>
  for ScanObserver<Source, BinaryOp, OutputItem>
where
  Source: Observer<OutputItem, Err>,
  BinaryOp: FnMut(OutputItem, InputItem) -> OutputItem,
  OutputItem: Clone,
{
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
  use crate::{ops::Scan, prelude::*};

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
    m.clone()
      .scan(|_acc, v| v as f32)
      .clone()
      .to_shared()
      .clone()
      .to_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn scan_fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(0..100).scan(|acc: i32, v| acc + v);
    m.clone()
      .scan(|acc: i32, v| acc + v)
      .clone()
      .to_shared()
      .clone()
      .to_shared()
      .subscribe(|_| {});
  }
}
