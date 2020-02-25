use crate::prelude::*;

pub trait BoxObservable<'a> {
  type Item;
  type Err;
  fn box_subscribe(
    self: Box<Self>,
    subscriber: Subscriber<
      Box<dyn Observer<Self::Item, Self::Err> + 'a>,
      LocalSubscription,
    >,
  ) -> Box<dyn SubscriptionLike>;
}

pub trait SharedBoxObservable {
  type Item;
  type Err;
  fn box_subscribe(
    self: Box<Self>,
    subscriber: Subscriber<
      Box<dyn Observer<Self::Item, Self::Err> + Send + Sync>,
      SharedSubscription,
    >,
  ) -> Box<dyn SubscriptionLike + Send + Sync>;
}

macro box_observable_impl(
  $subscription:ty, $source:ident, $($marker:ident +)* $lf: lifetime)
{
  type Item = $source::Item;
  type Err = $source::Err;
  fn box_subscribe(
    self: Box<Self>,
    subscriber: Subscriber<
      Box<dyn Observer<Self::Item, Self::Err> + $($marker +)* $lf>,
      $subscription,
    >,
  ) -> Box<dyn SubscriptionLike + $($marker +)*>  {
    Box::new(self.actual_subscribe(subscriber))
  }
}

impl<'a, T> BoxObservable<'a> for T
where
  T: Observable<'a> + 'a,
{
  box_observable_impl!(LocalSubscription, T, 'a);
}

impl<T> SharedBoxObservable for T
where
  T: SharedObservable,
  T::Unsub: Send + Sync,
  T::Item: Send + Sync + 'static,
  T::Err: Send + Sync + 'static,
{
  box_observable_impl!(SharedSubscription, T, Send + Sync + 'static);
}

pub struct BoxOp<T>(T);

pub type LocalBoxOp<'a, Item, Err> =
  BoxOp<Box<dyn BoxObservable<'a, Item = Item, Err = Err> + 'a>>;
pub type SharedBoxOp<Item, Err> =
  BoxOp<Box<dyn SharedBoxObservable<Item = Item, Err = Err> + Send + Sync>>;

macro observable_impl(  $subscription:ty, $($marker:ident +)* $lf: lifetime)
{
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + $($marker +)* $lf>(
    self,
    subscriber: Subscriber<O, $subscription>,
  ) -> Self::Unsub {
    self.0.box_subscribe(Subscriber {
      observer: Box::new(subscriber.observer),
      subscription: subscriber.subscription,
    })
  }
}

impl<'a, Item, Err> Observable<'a> for LocalBoxOp<'a, Item, Err>
where
  Item: 'a,
  Err: 'a,
{
  type Item = Item;
  type Err = Err;
  type Unsub = Box<dyn SubscriptionLike>;
  observable_impl!(LocalSubscription, 'a);
}

impl<Item, Err> SharedObservable for SharedBoxOp<Item, Err> {
  type Item = Item;
  type Err = Err;
  type Unsub = Box<dyn SubscriptionLike + Send + Sync>;
  observable_impl!(SharedSubscription, Send + Sync + 'static);
}

pub trait IntoBox<T> {
  fn box_it(origin: T) -> BoxOp<Self>
  where
    Self: Sized;
}

impl<'a, T> IntoBox<T>
  for Box<dyn BoxObservable<'a, Item = T::Item, Err = T::Err> + 'a>
where
  T: Observable<'a> + 'a,
{
  fn box_it(origin: T) -> BoxOp<Self> { BoxOp(Box::new(origin)) }
}

impl<T> IntoBox<T>
  for Box<dyn SharedBoxObservable<Item = T::Item, Err = T::Err> + Send + Sync>
where
  T: SharedObservable + Send + Sync + 'static,
  T::Item: Send + Sync + 'static,
  T::Err: Send + Sync + 'static,
  T::Unsub: Send + Sync,
{
  fn box_it(origin: T) -> BoxOp<Self> { BoxOp(Box::new(origin)) }
}

pub trait BoxIt {
  /// box an observable to a safety object and convert it to a simple type
  /// `BoxOp`, which only care `Item` and `Err` Observable emitted.
  ///
  /// # Example
  /// ```
  /// use rxrust::prelude::*;
  /// use ops::{box_it::LocalBoxOp, Map, BoxIt};
  ///
  /// let mut boxed: LocalBoxOp<'_, i32, ()> = observable::of(1)
  ///   .map(|v| v).box_it();
  ///
  /// // BoxOp can box any observable type
  /// boxed = observable::empty().box_it();
  ///
  /// boxed.subscribe(|_| {});
  /// ```
  fn box_it<O: IntoBox<Self>>(self) -> BoxOp<O>
  where
    Self: Sized,
  {
    O::box_it(self)
  }
}

impl<T> BoxIt for T {}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use ops::box_it::{BoxIt, LocalBoxOp, SharedBoxOp};

  #[test]
  fn box_observable() {
    let mut test = 0;
    let mut boxed: LocalBoxOp<'_, i32, ()> = observable::of(100).box_it();
    boxed.subscribe(|v| test = v);
    boxed = observable::empty().box_it();
    boxed.subscribe(|_| unreachable!());
    assert_eq!(test, 100);
  }
  #[test]
  fn shared_box_observable() {
    let mut boxed: SharedBoxOp<i32, ()> = observable::of(100).box_it();
    boxed.to_shared().subscribe(|_| {});
    boxed = observable::empty().box_it();
    boxed.to_shared().subscribe(|_| unreachable!());
  }
}
