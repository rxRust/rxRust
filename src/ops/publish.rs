/// Returns a ConnectableObservable. A ConnectableObservable Observable
/// resembles an ordinary Observable, except that it does not begin emitting
/// items when it is subscribed to, but only when the Connect operator is
/// applied to it. In this way you can wait for all intended observers to
/// subscribe to the Observable before the Observable begins emitting items.
///
use crate::observable::connectable_observable::ConnectableObservable;
pub use crate::prelude::*;
use subject::LocalPublishers;

pub trait Publish<'a, Item, Err>
where
  Self: Sized,
{
  fn publish(
    self,
  ) -> ConnectableObservable<
    Self,
    Subject<LocalPublishers<'a, Item, Err>, LocalSubscription>,
  > {
    ConnectableObservable::local(self)
  }
}

impl<'a, Item, Err, T> Publish<'a, Item, Err> for T {}

#[test]
fn smoke() {
  let p = observable::of!(100).publish();
  let mut first = 0;
  let mut second = 0;
  p.fork().subscribe(|v| first = *v);
  p.fork().subscribe(|v| second = *v);

  p.connect();
  assert_eq!(first, 100);
  assert_eq!(second, 100);
}
