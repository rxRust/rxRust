use crate::prelude::*;
use observable::ObservableFromFn;

/// Creates an observable that emitts no items, just terminates with an error.
///
/// # Arguments
///
/// * `e` - An error to emitt and terminate with
///
pub fn throw<O, U, Item, Err>(
  e: Err,
) -> ObservableFromFn<impl FnOnce(Subscriber<O, U>) + Clone, Item, Err>
where
  O: Observer<Item, Err>,
  U: SubscriptionLike,
  Err: Clone,
  Item: Clone,
{
  observable::create(move |mut subscriber| {
    subscriber.error(e);
  })
}

/// Creates an observable that produces no values.
///
/// Completes immediatelly. Never emits an error.
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
pub fn empty<O, U, Item>()
-> ObservableFromFn<impl FnOnce(Subscriber<O, U>) + Clone, Item, ()>
where
  O: Observer<Item, ()>,
  U: SubscriptionLike,
{
  observable::create(move |mut subscriber: Subscriber<O, U>| {
    subscriber.complete();
  })
}

/// Creates an observable that never emitts anything.
///
/// Neither emitts a value, nor completes, nor emitts an error.
///
pub fn never<O, U, Item>()
-> ObservableFromFn<impl FnOnce(Subscriber<O, U>) + Clone, Item, ()>
where
  O: Observer<Item, ()>,
  U: SubscriptionLike,
{
  observable::create(move |_subscriber: Subscriber<O, U>| {
    loop {
      // will not complete
      std::thread::sleep(std::time::Duration::from_secs(1));
    }
  })
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
      |_: i32| value_emitted = true,
      |e: String| error_emitted = e,
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
    observable::empty()
      .subscribe_complete(|_: &()| hits += 1, || completed = true);

    assert_eq!(hits, 0);
    assert_eq!(completed, true);
  }
}
