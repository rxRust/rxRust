use crate::prelude::*;
use std::marker::PhantomData;

/// Creates an observable that emits no items, just terminates with an error.
///
/// # Arguments
///
/// * `e` - An error to emit and terminate with
///
pub fn throw<Err>(e: Err) -> ObservableBase<ThrowEmitter<Err>> {
  ObservableBase::new(ThrowEmitter(e))
}

#[derive(Clone)]
pub struct ThrowEmitter<Err>(Err);

macro throw_emitter($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  #[inline]
  fn emit<O>(self, mut subscriber: Subscriber<O, $subscription>)
  where
    O: Observer<Self::Item, Self::Err> + $($marker +)* $lf
  {
    subscriber.error(self.0);
  }
}

impl<'a, Err> Emitter<'a> for ThrowEmitter<Err> {
  type Item = ();
  type Err = Err;
  throw_emitter!(LocalSubscription, 'a);
}

impl<Err> SharedEmitter for ThrowEmitter<Err> {
  type Item = ();
  type Err = Err;
  throw_emitter!(SharedSubscription, Send + Sync + 'static);
}

/// Creates an observable that produces no values.
///
/// Completes immediately. Never emits an error.
///
/// # Examples
/// ```
/// use rxrust::prelude::*;
///
/// observable::empty()
///   .subscribe(|v: &i32| {println!("{},", v)});
///
/// // Result: no thing printed
/// ```
///
pub fn empty<Item>() -> ObservableBase<EmptyEmitter<Item>> {
  ObservableBase::new(EmptyEmitter(PhantomData))
}

#[derive(Clone)]
pub struct EmptyEmitter<Item>(PhantomData<Item>);

macro empty_emitter($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  #[inline]
  fn emit<O>(self, mut subscriber: Subscriber<O, $subscription>)
  where
    O: Observer<Self::Item, Self::Err> + $($marker +)* $lf
  {
    subscriber.complete();
  }
}

impl<'a, Item> Emitter<'a> for EmptyEmitter<Item> {
  type Item = Item;
  type Err = ();
  empty_emitter!(LocalSubscription, 'a);
}

impl<Item> SharedEmitter for EmptyEmitter<Item> {
  type Item = Item;
  type Err = ();
  empty_emitter!(SharedSubscription, Send + Sync + 'static);
}
/// Creates an observable that never emits anything.
///
/// Neither emits a value, nor completes, nor emits an error.
///
pub fn never() -> ObservableBase<NeverEmitter> {
  ObservableBase::new(NeverEmitter())
}

#[derive(Clone)]
pub struct NeverEmitter();

macro never_emitter($subscription:ty, $($marker:ident +)* $lf: lifetime) {
  #[inline]
  fn emit<O>(self, subscriber: Subscriber<O, $subscription>)
  where
    O: Observer<Self::Item, Self::Err> + $($marker +)* $lf
  {
  }
}

impl<'a> Emitter<'a> for NeverEmitter {
  type Item = ();
  type Err = ();
  #[inline]
  never_emitter!(LocalSubscription, 'a);
}

impl SharedEmitter for NeverEmitter {
  type Item = ();
  type Err = ();
  #[inline]
  never_emitter!(SharedSubscription, Send + Sync + 'static);
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn throw() {
    let mut value_emitted = false;
    let mut completed = false;
    let mut error_emitted = String::new();
    observable::throw(String::from("error")).subscribe_all(
      // helping with type inference
      |_| value_emitted = true,
      |e| error_emitted = e,
      || completed = true,
    );
    assert!(!value_emitted);
    assert!(!completed);
    assert_eq!(error_emitted, "error");
  }

  #[test]
  fn empty() {
    let mut hits = 0;
    let mut completed = false;
    observable::empty().subscribe_complete(|()| hits += 1, || completed = true);

    assert_eq!(hits, 0);
    assert_eq!(completed, true);
  }
}
