/// Returns a ConnectableObservable. A ConnectableObservable Observable
/// resembles an ordinary Observable, except that it does not begin emitting
/// items when it is subscribed to, but only when the Connect operator is
/// applied to it. In this way you can wait for all intended observers to
/// subscribe to the Observable before the Observable begins emitting items.
///
pub use crate::prelude::*;
use observable::ConnectableObservable;

pub trait Publish {
  /// Returns a ConnectableObservable. A ConnectableObservable Observable
  /// resembles an ordinary Observable, except that it does not begin emitting
  /// items when it is subscribed to, but only when the Connect operator is
  /// applied to it. In this way you can wait for all intended observers to
  /// subscribe to the Observable before the Observable begins emitting items.
  ///
  fn publish<Subject: Default>(self) -> ConnectableObservable<Self, Subject>
  where
    Self: Sized,
  {
    ConnectableObservable {
      source: self,
      subject: Subject::default(),
    }
  }
}

impl<O> Publish for O {}
#[test]
fn smoke() {
  let p = observable::of(100).publish();
  let mut first = 0;
  let mut second = 0;
  let _guard1 = p.clone().subscribe(|v| first = v);
  let _guard2 = p.clone().subscribe(|v| second = v);

  p.connect();
  assert_eq!(first, 100);
  assert_eq!(second, 100);
}

#[test]
fn filter() {
  use crate::ops::FilterMap;
  use crate::subject::MutRefValue;
  let mut subject = Subject::local();

  subject
    .clone()
    .mut_ref_all()
    .filter_map::<fn(&mut i32) -> Option<&mut i32>, _, _>(|v| Some(v))
    .publish()
    .subscribe_err(|_: &mut _| {}, |_: &mut i32| {});

  subject.next(MutRefValue(&mut 1));
  subject.error(MutRefValue(&mut 2));
}
