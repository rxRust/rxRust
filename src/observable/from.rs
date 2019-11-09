use crate::prelude::*;

/// Creates an observable that produces values from an iterator.
///
/// Completes when all elements have been emitted. Never emits an error.
///
/// # Examples
///
/// A simple example for a range:
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::from_iter(0..10)
///   .subscribe(|v| {println!("{},", v)});
/// ```
///
/// Or with a vector:
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::from_iter(vec![0,1,2,3])
///   .subscribe(|v| {println!("{},", v)});
/// ```
///
pub fn from_iter<O, U, Iter>(
  iter: Iter,
) -> Observable<impl FnOnce(Subscriber<O, U>) + Clone>
where
  O: Observer<<Iter as IntoIterator>::Item, ()>,
  U: SubscriptionLike,
  Iter: IntoIterator + Clone,
{
  Observable::new(move |mut subscriber| {
    for v in iter.into_iter() {
      if !subscriber.is_closed() {
        subscriber.next(&v);
      } else {
        break;
      }
    }
    if !subscriber.is_closed() {
      subscriber.complete();
    }
  })
}

/// Creates an observable producing a single value.
///
/// Completes immediatelly after emitting the value given. Never emits an error.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::of(123)
///   .subscribe(|v| {println!("{},", v)});
/// ```
///
pub fn of<O, U, Item>(
  v: Item,
) -> Observable<impl FnOnce(Subscriber<O, U>) + Clone>
where
  O: Observer<Item, ()>,
  U: SubscriptionLike,
  Item: Clone,
{
  Observable::new(move |mut subscriber| {
    subscriber.next(&v);
    subscriber.complete();
  })
}

/// Creates an observable that emits value or the error from a [`Result`] given.
///
/// Completes immediatelly after.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::of_result(Ok(1234))
///   .subscribe(|v| {println!("{},", v)});
/// ```
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::of_result(Err("An error"))
///   .subscribe_err(|v: &i32| {}, |e| {println!("Error:  {},", e)});
/// ```
///
pub fn of_result<O, U, Item, Err>(
  r: Result<Item, Err>,
) -> Observable<impl FnOnce(Subscriber<O, U>) + Clone>
where
  O: Observer<Item, Err>,
  U: SubscriptionLike,
  Item: Clone,
  Err: Clone,
{
  Observable::new(move |mut subscriber| {
    match &r {
      Ok(v) => subscriber.next(v),
      Err(e) => subscriber.error(e),
    };
    subscriber.complete();
  })
}

/// Creates an observable that potentially emits a single value from [`Option`].
///
/// Emits the value if is there, and completes immediatelly after. When the
/// given option has not value, completes immediatelly. Never emitts an error.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::of_option(Some(1234))
///   .subscribe(|v| {println!("{},", v)});
/// ```
///
pub fn of_option<O, U, Item>(
  o: Option<Item>,
) -> Observable<impl FnOnce(Subscriber<O, U>) + Clone>
where
  O: Observer<Item, ()>,
  U: SubscriptionLike,
  Item: Clone,
{
  Observable::new(move |mut subscriber| {
    match &o {
      Some(v) => subscriber.next(v),
      None => (),
    };
    subscriber.complete();
  })
}

/// Creates an observable that emits the return value of a callable.
///
/// Never emits an error
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::from_fn(|| {1234})
///   .subscribe(|v| {println!("{},", v)});
/// ```
///
pub fn from_fn<O, U, Callable, Item>(
  callable: Callable,
) -> Observable<impl FnOnce(Subscriber<O, U>) + Clone>
where
  Callable: FnOnce() -> Item,
  O: Observer<Item, ()>,
  U: SubscriptionLike,
  Item: Clone,
{
  // Because of Rust zero-cost abstraction we can compose from
  // what we already have without a fear of too much overhead added.
  of(callable())
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn from_fn() {
    let mut value = 0;
    let mut completed = false;
    let callable = || 123;
    observable::from_fn(callable).subscribe_complete(
      |v| {
        value = *v;
      },
      || completed = true,
    );

    assert_eq!(value, 123);
    assert!(completed);
  }

  #[test]
  fn of_option() {
    let mut value1 = 0;
    let mut completed1 = false;
    observable::of_option(Some(123)).subscribe_complete(
      |v| {
        value1 = *v;
      },
      || completed1 = true,
    );

    assert_eq!(value1, 123);
    assert!(completed1);

    let mut value2 = 0;
    let mut completed2 = false;
    observable::of_option(None).subscribe_complete(
      |v| {
        value2 = *v;
      },
      || completed2 = true,
    );

    assert_eq!(value2, 0);
    assert!(completed2);
  }

  #[test]
  fn of_result() {
    let mut value1 = 0;
    let mut completed1 = false;
    let r: Result<i32, &str> = Ok(123);
    observable::of_result(r).subscribe_complete(
      |v| {
        value1 = *v;
      },
      || completed1 = true,
    );

    assert_eq!(value1, 123);
    assert!(completed1);

    let mut value2 = 0;
    let mut error_reported = false;
    let r: Result<i32, &str> = Err("error");
    observable::of_result(r)
      .subscribe_err(|_| value2 = 123, |_| error_reported = true);

    assert_eq!(value2, 0);
    assert!(error_reported);
  }

  #[test]
  fn from_range() {
    let mut hit_count = 0;
    let mut completed = false;
    observable::from_iter(0..100)
      .subscribe_complete(|_| hit_count += 1, || completed = true);

    assert_eq!(hit_count, 100);
    assert_eq!(completed, true);
  }

  #[test]
  fn from_vec() {
    let mut hit_count = 0;
    let mut completed = false;
    observable::from_iter(vec![0; 100])
      .subscribe_complete(|_| hit_count += 1, || completed = true);

    assert_eq!(hit_count, 100);
    assert_eq!(completed, true);
  }

  #[test]
  fn of() {
    let mut value = 0;
    let mut completed = false;
    observable::of(100).subscribe_complete(|v| value = *v, || completed = true);

    assert_eq!(value, 100);
    assert_eq!(completed, true);
  }

  #[test]
  fn fork() {
    use crate::ops::Fork;

    observable::from_iter(vec![0; 100])
      .fork()
      .fork()
      .subscribe(|_| {});

    observable::of(0).fork().fork().subscribe(|_| {});
  }
}
