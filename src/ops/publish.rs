/// Returns a ConnectableObservable. A ConnectableObservable Observable
/// resembles an ordinary Observable, except that it does not begin emitting
/// items when it is subscribed to, but only when the Connect operator is
/// applied to it. In this way you can wait for all intended observers to
/// subscribe to the Observable before the Observable begins emitting items.
///
use crate::observable::connectable_observable::ConnectableObservable;
pub use crate::prelude::*;
use crate::subject::LocalObserver;

pub trait Publish
where
  Self: Sized,
{
  #[inline(always)]
  fn publish<'a, Item, Err>(
    self,
  ) -> ConnectableObservable<Self, LocalSubject<'a, Item, Err>> {
    self.publish_raw()
  }

  /// publish_raw let you can give an explicit box `Publisher` type to the
  /// `ConnectableObservable`, for most scenes use `publish` is enough, but when
  /// you pass a mut reference item or error, you can use `publish_raw` to give
  /// a box trait type for `Subject`
  /// ```ignore
  /// use rxrust::prelude::*;
  /// use rxrust::observable::Connect;
  /// use rxrust::subscription::Publisher;
  /// use rxrust::ops::{Publish};
  ///
  /// let p = observable::of(100)
  ///   .publish_raw::<Box<dyn for<'r> Publisher<&'r mut i32, _>>>();
  /// let mut first = 0;
  /// let mut second = 0;
  /// p.fork().subscribe(|v| first = *v);
  /// p.fork().subscribe(|v| second = *v);
  /// ```
  #[inline(always)]
  fn publish_raw<P>(
    self,
  ) -> ConnectableObservable<Self, Subject<LocalObserver<P>, LocalSubscription>>
  {
    ConnectableObservable::local(self)
  }
}

impl<T> Publish for T {}

#[test]
fn smoke() {
  use crate::observable::Connect;
  let p = observable::of(100).publish();
  let mut first = 0;
  let mut second = 0;
  p.fork().subscribe(|v| first = v);
  p.fork().subscribe(|v| second = v);

  p.connect();
  assert_eq!(first, 100);
  assert_eq!(second, 100);
}
