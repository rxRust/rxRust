use crate::prelude::*;

#[derive(Clone)]
pub struct ScanOp<Source, BinaryOp, OutputItem, InputItem> {
  source: Source,
  binary_op: BinaryOp,
  initial_value: OutputItem,
  _m: TypeHint<InputItem>,
}

impl<Source, BinaryOp, OutputItem, InputItem>
  ScanOp<Source, BinaryOp, OutputItem, InputItem>
{
  pub(crate) fn new(
    source: Source,
    binary_op: BinaryOp,
    initial_value: OutputItem,
  ) -> Self {
    Self {
      source,
      binary_op,
      initial_value,
      _m: TypeHint::default(),
    }
  }
}

pub struct ScanObserver<Observer, BinaryOp, OutputItem> {
  target_observer: Observer,
  binary_op: BinaryOp,
  acc: OutputItem,
}

impl<InputItem, OutputItem, Err, O, S, BinaryOp> Observable<OutputItem, Err, O>
  for ScanOp<S, BinaryOp, OutputItem, InputItem>
where
  S: Observable<InputItem, Err, ScanObserver<O, BinaryOp, OutputItem>>,
  O: Observer<OutputItem, Err>,
  BinaryOp: FnMut(OutputItem, InputItem) -> OutputItem,
  OutputItem: Clone,
{
  type Unsub = S::Unsub;
  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self.source.actual_subscribe(ScanObserver {
      target_observer: observer,
      binary_op: self.binary_op,
      acc: self.initial_value,
    })
  }
}

impl<InputItem, OutputItem, Err, S, BinaryOp> ObservableExt<OutputItem, Err>
  for ScanOp<S, BinaryOp, OutputItem, InputItem>
where
  S: ObservableExt<InputItem, Err>,
{
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

  #[inline]
  fn error(self, err: Err) {
    self.target_observer.error(err)
  }

  #[inline]
  fn complete(self) {
    self.target_observer.complete()
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.target_observer.is_finished()
  }
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
    m.scan(|_acc, v| v as f32).subscribe(|_| {});
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_scan);

  fn bench_scan(b: &mut bencher::Bencher) {
    b.iter(scan_initial);
  }
}
