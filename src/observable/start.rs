use crate::{impl_local_shared_both, prelude::*};

pub fn start<Item>(
  func: impl FnOnce() -> Item + 'static,
) -> StartObservable<Item> {
  StartObservable {
    func: Box::new(func),
  }
}

pub struct StartObservable<Item> {
  func: Box<dyn FnOnce() -> Item>,
}

impl<Item> Observable for StartObservable<Item> {
  type Item = Item;
  type Err = ();
}

impl_local_shared_both! {
  impl<Item> StartObservable<Item>;
  type Unsub = SingleSubscription;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let result = ($self.func)();
    $observer.next(result);
    $observer.complete();
    SingleSubscription::default()
  }
}

#[cfg(test)]
mod tests {
  use crate::prelude::*;
  use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
  use std::sync::Arc;

  #[test]
  fn it_shall_emit_closure_value() {
    let mut actual = 0;
    let mut is_completed = false;

    observable::start(|| 123).subscribe_all(
      |n| actual = n,
      |_| {},
      || is_completed = true,
    );

    assert_eq!(123, actual);
    assert!(is_completed);
  }

  #[test]
  fn it_shall_emit_closure_value_shared() {
    let actual = Arc::new(AtomicI32::new(0));
    let is_completed = Arc::new(AtomicBool::new(false));

    let actual_c = actual.clone();
    let is_completed_c = is_completed.clone();
    observable::start(|| 123)
      .into_shared()
      .subscribe_blocking_all(
        move |n| actual_c.store(n, Ordering::Relaxed),
        |_| {},
        move || is_completed_c.store(true, Ordering::Relaxed),
      );

    assert_eq!(123, actual.load(Ordering::Relaxed));
    assert!(is_completed.load(Ordering::Relaxed));
  }

  #[derive(PartialEq, Debug)]
  struct S {
    i: i32,
    f: f32,
    s: String,
  }

  fn function() -> S {
    S {
      i: 1,
      f: 2.5,
      s: String::from("aString"),
    }
  }

  #[test]
  fn it_shall_emit_function_value() {
    let expected = S {
      i: 1,
      f: 2.5,
      s: String::from("aString"),
    };
    let mut actual = S {
      i: 0,
      f: 0.0,
      s: String::new(),
    };
    let mut is_completed = false;

    observable::start(function).subscribe_all(
      |n| actual = n,
      |_| {},
      || is_completed = true,
    );

    assert_eq!(expected, actual);
    assert!(is_completed);
  }
}
