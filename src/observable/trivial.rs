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

impl<Err> Emitter for ThrowEmitter<Err> {
  type Item = ();
  type Err = Err;
  #[inline]
  fn emit<O, U>(self, mut subscriber: Subscriber<O, U>)
  where
    O: Observer<Self::Item, Self::Err>,
    U: SubscriptionLike,
  {
    subscriber.error(self.0);
  }
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

impl<Item> Emitter for EmptyEmitter<Item> {
  type Item = Item;
  type Err = ();
  #[inline]
  fn emit<O, U>(self, mut subscriber: Subscriber<O, U>)
  where
    O: Observer<Self::Item, Self::Err>,
    U: SubscriptionLike,
  {
    subscriber.complete();
  }
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

impl Emitter for NeverEmitter {
  type Item = ();
  type Err = ();
  #[inline]
  fn emit<O, U>(self, _subscriber: Subscriber<O, U>)
  where
    O: Observer<Self::Item, Self::Err>,
    U: SubscriptionLike,
  {
  }
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
