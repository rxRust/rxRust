use crate::prelude::*;
use ops::SharedOp;
use std::marker::PhantomData;

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
  ) -> ScanOp<Self, BinaryOp, InputItem, OutputItem>
  where
    Self: Sized,
    BinaryOp: Fn(&OutputItem, &InputItem) -> OutputItem,
  {
    ScanOp {
      source_observable: self,
      binary_op,
      initial_value,
      _p: PhantomData,
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
  ) -> ScanOp<Self, BinaryOp, InputItem, OutputItem>
  where
    Self: Sized,
    BinaryOp: Fn(&OutputItem, &InputItem) -> OutputItem,
    OutputItem: Default,
  {
    self.scan_initial(OutputItem::default(), binary_op)
  }
}

impl<O, OutputItem> Scan<OutputItem> for O {}

pub struct ScanOp<Source, BinaryOp, InputItem, OutputItem> {
  source_observable: Source,
  binary_op: BinaryOp,
  initial_value: OutputItem,
  _p: PhantomData<InputItem>,
}

pub struct ScanObserver<Observer, BinaryOp, OutputItem> {
  target_observer: Observer,
  binary_op: BinaryOp,
  acc: OutputItem,
}

// We're making the `ScanOp` being an publisher - an object that
// subscribers can subscribe to.
// Doing so by implementing `RawSubscribable` trait for it.
// Once instantiated, it will have a `raw_subscribe` method in it.
// Note: we're accepting such subscribers that accept `Output` as their
// `Item` type.
impl<OutputItem, Err, InputItem, Observer, Subscription, Source, BinaryOp>
  RawSubscribable<OutputItem, Err, Subscriber<Observer, Subscription>>
  for ScanOp<Source, BinaryOp, InputItem, OutputItem>
where
  Source: RawSubscribable<
    InputItem,
    Err,
    Subscriber<ScanObserver<Observer, BinaryOp, OutputItem>, Subscription>,
  >,
  BinaryOp: FnMut(&OutputItem, &InputItem) -> OutputItem,
{
  type Unsub = Source::Unsub;
  fn raw_subscribe(
    self,
    subscriber: Subscriber<Observer, Subscription>,
  ) -> Self::Unsub {
    self.source_observable.raw_subscribe(Subscriber {
      observer: ScanObserver {
        target_observer: subscriber.observer,
        binary_op: self.binary_op,
        acc: self.initial_value,
      },
      subscription: subscriber.subscription,
    })
  }
}

// We're making `ScanObserver` being able to be subscribed to other observables
// by implementing `Observer` trait. Thanks to this, it is able to observe
// sources having `Item` type as its `InputItem` type.
impl<InputItem, Err, Source, BinaryOp, OutputItem> Observer<InputItem, Err>
  for ScanObserver<Source, BinaryOp, OutputItem>
where
  Source: Observer<OutputItem, Err>,
  BinaryOp: FnMut(&OutputItem, &InputItem) -> OutputItem,
{
  fn next(&mut self, value: &InputItem) {
    // accumulating each item with a current value
    self.acc = (self.binary_op)(&self.acc, value);
    self.target_observer.next(&self.acc)
  }

  #[inline(always)]
  fn error(&mut self, err: &Err) { self.target_observer.error(err); }

  #[inline(always)]
  fn complete(&mut self) { self.target_observer.complete(); }
}

impl<Source, BinaryOp, InputItem, OutputItem> Fork
  for ScanOp<Source, BinaryOp, InputItem, OutputItem>
where
  Source: Fork,
  BinaryOp: Clone,
  OutputItem: Clone,
{
  type Output = ScanOp<Source::Output, BinaryOp, InputItem, OutputItem>;
  fn fork(&self) -> Self::Output {
    ScanOp {
      source_observable: self.source_observable.fork(),
      binary_op: self.binary_op.clone(),
      initial_value: self.initial_value.clone(),
      _p: self._p,
    }
  }
}

impl<Source, BinaryOp, OutputItem> IntoShared
  for ScanObserver<Source, BinaryOp, OutputItem>
where
  Source: IntoShared,
  BinaryOp: Send + Sync + 'static,
  OutputItem: Send + Sync + 'static,
{
  type Shared = ScanObserver<Source::Shared, BinaryOp, OutputItem>;
  fn to_shared(self) -> Self::Shared {
    ScanObserver {
      target_observer: self.target_observer.to_shared(),
      binary_op: self.binary_op,
      acc: self.acc,
    }
  }
}

impl<Source, BinaryOp, InputItem, OutputItem> IntoShared
  for ScanOp<Source, BinaryOp, InputItem, OutputItem>
where
  Source: IntoShared,
  BinaryOp: Send + Sync + 'static,
  InputItem: Send + Sync + 'static,
  OutputItem: Send + Sync + 'static,
{
  type Shared =
    SharedOp<ScanOp<Source::Shared, BinaryOp, InputItem, OutputItem>>;
  fn to_shared(self) -> Self::Shared {
    SharedOp(ScanOp {
      source_observable: self.source_observable.to_shared(),
      binary_op: self.binary_op,
      initial_value: self.initial_value,
      _p: self._p,
    })
  }
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
      .subscribe(|v| emitted.push(*v));

    assert_eq!(vec!(101, 102, 103, 104, 105), emitted);
  }

  #[test]
  fn scan_initial_mixed_types() {
    let mut emitted = Vec::<i32>::new();
    // Should work like accumulate from 100,
    // as we ignore the input characters and just
    // increment the accumulated value given.
    observable::from_iter(vec!['a', 'b', 'c', 'd', 'e'])
      .scan_initial(100, |acc, _v| acc + 1)
      .subscribe(|v| emitted.push(*v));

    assert_eq!(vec!(101, 102, 103, 104, 105), emitted);
  }

  #[test]
  fn scan_with_default() {
    let mut emitted = Vec::<i32>::new();
    // should work like accumulate from 0
    observable::from_iter(vec![1, 1, 1, 1, 1])
      .scan(|acc, v| acc + v)
      .subscribe(|v| emitted.push(*v));

    assert_eq!(vec!(1, 2, 3, 4, 5), emitted);
  }

  #[test]
  fn scan_fork_and_shared_mixed_types() {
    // type to type can fork
    let m = observable::from_iter(vec!['a', 'b', 'c']).scan(|_acc, _v| 1i32);
    m.fork()
      .scan(|_acc, v| *v as f32)
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_| {});
  }

  #[test]
  fn scan_fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter(0..100).scan(|acc, v| acc + *v);
    m.fork()
      .scan(|acc, v| acc + *v)
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_| {});
  }
}
