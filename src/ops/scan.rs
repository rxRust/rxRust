use crate::prelude::*;
use ops::SharedOp;

/// The Scan operator applies a function to the first item emitted by the
/// source observable and then emits the result of that function as its
/// own first emission. It also feeds the result of the function back into
/// the function along with the second item emitted by the source observable
/// in order to generate its second emission. It continues to feed back its
/// own subsequent emissions along with the subsequent emissions from the
/// source Observable in order to create the rest of its sequence.
pub trait Scan<T> {
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
  /// * `initial` - An initial value to start the successive accumulations from.
  /// * `f` - A closure acting as a binary operator.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  /// use rxrust::ops::Scan;
  ///
  /// observable::from_iter!(vec!(1, 1, 1, 1, 1))
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
  fn scan_initial<B, F>(self, initial: B, f: F) -> ScanOp<Self, F, B>
  where
    Self: Sized,
    F: Fn(&B, &T) -> B,
  {
    ScanOp {
      source: self,
      func: f,
      acc: initial,
    }
  }

  /// Works like [`scan_initial`] but starts with a value defined by a
  /// [`Default`] trait for the first argument `f` operator operates on.
  ///
  /// # Arguments
  ///
  /// * `f` - A closure acting as a binary operator.
  ///
  fn scan<B, F>(self, f: F) -> ScanOp<Self, F, B>
  where
    Self: Sized,
    F: Fn(&B, &T) -> B,
    B: Default,
  {
    self.scan_initial(B::default(), f)
  }
}

impl<O, Item> Scan<Item> for O {}

pub struct ScanOp<S, F, B> {
  source: S,
  func: F,
  acc: B,
}

impl<Item, Err, O, U, S, B, M> RawSubscribable<Item, Err, Subscriber<O, U>>
  for ScanOp<S, M, B>
where
  S: RawSubscribable<B, Err, Subscriber<ScanSubscribe<O, M, B>, U>>,
  M: FnMut(&B, &Item) -> B,
{
  type Unsub = S::Unsub;
  fn raw_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
    let initial = self.acc;
    self.source.raw_subscribe(Subscriber {
      observer: ScanSubscribe {
        observer: subscriber.observer,
        func: self.func,
        acc: initial,
      },
      subscription: subscriber.subscription,
    })
  }
}

pub struct ScanSubscribe<S, M, B> {
  observer: S,
  func: M,
  acc: B,
}

impl<Item, Err, S, M, B> Observer<Item, Err> for ScanSubscribe<S, M, B>
where
  S: Observer<B, Err>,
  M: FnMut(&B, &Item) -> B,
{
  fn next(&mut self, value: &Item) {
    // accumulating each item with a current value
    self.acc = (self.func)(&self.acc, value);
    self.observer.next(&self.acc)
  }

  #[inline(always)]
  fn error(&mut self, err: &Err) { self.observer.error(err); }

  #[inline(always)]
  fn complete(&mut self) { self.observer.complete(); }
}

impl<S, M, B> Fork for ScanOp<S, M, B>
where
  S: Fork,
  M: Clone,
  B: Clone,
{
  type Output = ScanOp<S::Output, M, B>;
  fn fork(&self) -> Self::Output {
    ScanOp {
      source: self.source.fork(),
      func: self.func.clone(),
      acc: self.acc.clone(),
    }
  }
}

impl<S, M, B> IntoShared for ScanSubscribe<S, M, B>
where
  S: IntoShared,
  M: Send + Sync + 'static,
  B: Send + Sync + 'static,
{
  type Shared = ScanSubscribe<S::Shared, M, B>;
  fn to_shared(self) -> Self::Shared {
    ScanSubscribe {
      observer: self.observer.to_shared(),
      func: self.func,
      acc: self.acc,
    }
  }
}

impl<S, M, B> IntoShared for ScanOp<S, M, B>
where
  S: IntoShared,
  M: Send + Sync + 'static,
  B: Send + Sync + 'static,
{
  type Shared = SharedOp<ScanOp<S::Shared, M, B>>;
  fn to_shared(self) -> Self::Shared {
    SharedOp(ScanOp {
      source: self.source.to_shared(),
      func: self.func,
      acc: self.acc,
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
    observable::from_iter!(vec!(1, 1, 1, 1, 1))
      .scan_initial(100, |acc, v| acc + v)
      .subscribe(|v| emitted.push(*v));

    assert_eq!(vec!(101, 102, 103, 104, 105), emitted);
  }

  #[test]
  fn scan_with_default() {
    let mut emitted = Vec::<i32>::new();
    // should work like accumulate from 0
    observable::from_iter!(vec!(1, 1, 1, 1, 1))
      .scan(|acc, v| acc + v)
      .subscribe(|v| emitted.push(*v));

    assert_eq!(vec!(1, 2, 3, 4, 5), emitted);
  }

  #[test]
  fn scan_fork_and_shared() {
    // type to type can fork
    let m = observable::from_iter!(0..100).scan(|acc, v| acc + *v);
    m.fork()
      .scan(|acc, v| acc + *v)
      .fork()
      .to_shared()
      .fork()
      .to_shared()
      .subscribe(|_| {});
  }
}
