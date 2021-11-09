use crate::{impl_local_shared_both, prelude::*};

/// Creates an observable that emits no items, just terminates with an error.
///
/// # Arguments
///
/// * `e` - An error to emit and terminate with
pub fn throw<Err>(e: Err) -> ThrowObservable<Err> { ThrowObservable(e) }

#[derive(Clone)]
pub struct ThrowObservable<Err>(Err);

impl<Err> Observable for ThrowObservable<Err> {
  type Item = ();
  type Err = Err;
}

impl_local_shared_both! {
 impl<Err> ThrowObservable<Err>;
 type Unsub = SingleSubscription;
 macro method($self:ident, $observer: ident, $ctx: ident) {
   $observer.error($self.0);
   SingleSubscription::default()
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
#[inline]
pub fn empty<Item>() -> EmptyObservable<Item> {
  EmptyObservable(TypeHint::new())
}

#[derive(Clone)]
pub struct EmptyObservable<Item>(TypeHint<Item>);

impl<Item> Observable for EmptyObservable<Item> {
  type Item = Item;
  type Err = ();
}

impl_local_shared_both! {
  impl<Item> EmptyObservable<Item>;
  type Unsub = SingleSubscription;
  macro method($self:ident, $observer: ident, $ctx: ident) {
    $observer.complete();
    SingleSubscription::default()
  }
}

/// Creates an observable that never emits anything.
///
/// Neither emits a value, nor completes, nor emits an error.
#[inline]
pub fn never() -> NeverObservable { NeverObservable }

#[derive(Clone)]
pub struct NeverObservable;

impl Observable for NeverObservable {
  type Item = ();
  type Err = ();
}

impl_local_shared_both! {
  impl NeverObservable;
  type Unsub = SingleSubscription;
  macro method($self:ident, $observer: ident, $ctx: ident) {
    SingleSubscription::default()
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
    assert!(completed);
  }
}
