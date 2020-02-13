use crate::observer::error_proxy_impl;
use crate::prelude::*;

/// Emits a single last item emitted by the source observable.
/// The item is emitted after source observable has completed.
///
pub trait Last<Item> {
  /// Emit only the last final item emitted by a source observable or a
  /// default item given.
  ///
  /// Completes right after emitting the single item. Emits error when
  /// source observable emits it.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  /// use rxrust::ops::Last;
  ///
  /// observable::empty()
  ///   .last_or(1234)
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 1234
  /// ```
  ///
  fn last_or(self, default: Item) -> LastOrOp<Self, Item>
  where
    Self: Sized,
  {
    LastOrOp {
      source: self,
      default: Some(default),
      last: None,
    }
  }

  /// Emits only last final item emitted by a source observable.
  ///
  /// Completes right after emitting the single last item, or when source
  /// observable completed, being an empty one. Emits error when source
  /// observable emits it.
  ///
  /// # Examples
  ///
  /// ```
  /// use rxrust::prelude::*;
  /// use rxrust::ops::Last;
  ///
  /// observable::from_iter(0..100)
  ///   .last()
  ///   .subscribe(|v| println!("{}", v));
  ///
  /// // print log:
  /// // 99
  /// ```
  ///
  fn last(self) -> LastOrOp<Self, Item>
  where
    Self: Sized,
  {
    LastOrOp {
      source: self,
      default: None,
      last: None,
    }
  }
}

impl<Item, O> Last<Item> for O {}

#[derive(Clone)]
pub struct LastOrOp<S, Item> {
  source: S,
  default: Option<Item>,
  last: Option<Item>,
}

macro observable_impl($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
    let subscriber = Subscriber {
      observer: LastOrObserver {
        observer: subscriber.observer,
        default: self.default,
        last: self.last,
      },
      subscription: subscriber.subscription,
    };
    self.source.actual_subscribe(subscriber)
  }
}

impl<'a, Item, S> Observable<'a> for LastOrOp<S, Item>
where
  S: Observable<'a, Item = Item>,
  Item: 'a,
{
  type Item = Item;
  type Err = S::Err;
  type Unsub = S::Unsub;
  observable_impl!(LocalSubscription, 'a);
}

impl<Item, S> SharedObservable for LastOrOp<S, Item>
where
  S: SharedObservable<Item = Item>,
  Item: Send + Sync + 'static,
{
  type Item = Item;
  type Err = S::Err;
  type Unsub = S::Unsub;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

pub struct LastOrObserver<S, T> {
  default: Option<T>,
  observer: S,
  last: Option<T>,
}

impl<O, Item, Err> Observer<Item, Err> for LastOrObserver<O, Item>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, value: Item) { self.last = Some(value); }
  error_proxy_impl!(Err, observer);
  fn complete(&mut self) {
    if let Some(v) = self.last.take().or_else(|| self.default.take()) {
      self.observer.next(v)
    }
    self.observer.complete();
  }
}

#[cfg(test)]
mod test {
  use super::Last;
  use crate::prelude::*;

  #[test]
  fn last_or_hundered_items() {
    let mut completed = 0;
    let mut errors = 0;
    let mut last_item = None;

    observable::from_iter(0..100).last_or(200).subscribe_all(
      |v| last_item = Some(v),
      |_| errors += 1,
      || completed += 1,
    );

    assert_eq!(errors, 0);
    assert_eq!(completed, 1);
    assert_eq!(Some(99), last_item);
  }

  #[test]
  fn last_or_no_items() {
    let mut completed = 0;
    let mut errors = 0;
    let mut last_item = None;

    observable::empty().last_or(100).subscribe_all(
      |v| last_item = Some(v),
      |_| errors += 1,
      || completed += 1,
    );

    assert_eq!(errors, 0);
    assert_eq!(completed, 1);
    assert_eq!(Some(100), last_item);
  }

  #[test]
  fn last_one_item() {
    let mut completed = 0;
    let mut errors = 0;
    let mut last_item = None;

    observable::from_iter(0..2).last().subscribe_all(
      |v| last_item = Some(v),
      |_| errors += 1,
      || completed += 1,
    );

    assert_eq!(errors, 0);
    assert_eq!(completed, 1);
    assert_eq!(Some(1), last_item);
  }

  #[test]
  fn last_no_items() {
    let mut completed = 0;
    let mut errors = 0;
    let mut last_item = None;

    observable::empty().last().subscribe_all(
      |v: i32| last_item = Some(v),
      |_| errors += 1,
      || completed += 1,
    );

    assert_eq!(errors, 0);
    assert_eq!(completed, 1);
    assert_eq!(None, last_item);
  }

  #[test]
  fn last_support_fork() {
    let mut value = 0;
    let mut value2 = 0;
    {
      let o = observable::from_iter(1..100).last();
      let o1 = o.clone().last();
      let o2 = o.clone().last();
      o1.subscribe(|v| value = v);
      o2.subscribe(|v| value2 = v);
    }
    assert_eq!(value, 99);
    assert_eq!(value2, 99);
  }

  #[test]
  fn last_or_support_fork() {
    let mut default = 0;
    let mut default2 = 0;
    let o = observable::create(|mut subscriber| {
      subscriber.complete();
    })
    .last_or(100);
    let o1 = o.clone().last_or(0);
    let o2 = o.clone().last_or(0);
    o1.subscribe(|v| default = v);
    o2.subscribe(|v| default2 = v);
    assert_eq!(default, 100);
    assert_eq!(default, 100);
  }

  #[test]
  fn last_fork_and_shared() {
    observable::of(0)
      .last_or(0)
      .clone()
      .clone()
      .to_shared()
      .clone()
      .to_shared()
      .subscribe(|_| {});

    observable::of(0)
      .last()
      .clone()
      .clone()
      .to_shared()
      .clone()
      .to_shared()
      .subscribe(|_| {});
  }
}
