use crate::prelude::*;
use observable::ObservableFromFn;

/// Creates an observable that produces values from an iterator.
///
/// Completes when all elements have been emitted. Never emits an error.
///
/// # Arguments
///
/// * `iter` - An iterator to get all the values from.
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
pub fn from_iter<O, U, Iter, I>(
  iter: Iter,
) -> ObservableFromFn<impl FnOnce(Subscriber<O, U>) + Clone, I, ()>
where
  O: Observer<I, ()>,
  U: SubscriptionLike,
  Iter: IntoIterator<Item = I> + Clone,
{
  observable::create(move |mut subscriber| {
    for v in iter.into_iter() {
      if !subscriber.is_closed() {
        subscriber.next(v);
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
/// # Arguments
///
/// * `v` - A value to emitt.
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
) -> ObservableFromFn<impl FnOnce(Subscriber<O, U>) + Clone, Item, ()>
where
  O: Observer<Item, ()>,
  U: SubscriptionLike,
  Item: Clone,
{
  observable::create(move |mut subscriber| {
    subscriber.next(v);
    subscriber.complete();
  })
}

/// Creates an observable producing same value repeated N times.
///
/// Completes immediatelly after emitting N values. Never emits an error.
///
/// # Arguments
///
/// * `v` - A value to emitt.
/// * `n` - A number of time to repeat it.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::repeat(123, 3)
///   .subscribe(|v| {println!("{},", v)});
///
/// // print log:
/// // 123
/// // 123
/// // 123
/// ```
///
pub fn repeat<O, U, Item>(
  v: Item,
  n: usize,
) -> ObservableFromFn<impl FnOnce(Subscriber<O, U>) + Clone, Item, ()>
where
  O: Observer<Item, ()>,
  U: SubscriptionLike,
  Item: Clone,
{
  from_iter(std::iter::repeat(v).take(n))
}

/// Creates an observable that emits value or the error from a [`Result`] given.
///
/// Completes immediatelly after.
///
/// # Arguments
///
/// * `r` - A [`Result`] argument to take a value, or an error to emitt from.
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
) -> ObservableFromFn<impl FnOnce(Subscriber<O, U>) + Clone, Item, Err>
where
  O: Observer<Item, Err>,
  U: SubscriptionLike,
  Item: Clone,
  Err: Clone,
{
  observable::create(move |mut subscriber| {
    match r {
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
/// # Arguments
///
/// * `o` - An optional used to take a value to emitt from.
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
) -> ObservableFromFn<impl FnOnce(Subscriber<O, U>) + Clone, Item, ()>
where
  O: Observer<Item, ()>,
  U: SubscriptionLike,
  Item: Clone,
{
  observable::create(move |mut subscriber| {
    if let Some(v) = o {
      subscriber.next(v)
    }
    subscriber.complete();
  })
}

/// Creates an observable that emits the return value of a callable.
///
/// Never emits an error.
///
/// # Arguments
///
/// * `f` - A function that will be called to obtain its return value to emitt.
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
  f: Callable,
) -> ObservableFromFn<impl FnOnce(Subscriber<O, U>) + Clone, Item, ()>
where
  Callable: FnOnce() -> Item,
  O: Observer<Item, ()>,
  U: SubscriptionLike,
  Item: Clone,
{
  // Because of Rust zero-cost abstraction we can compose from
  // what we already have without a fear of too much overhead added.
  of(f())
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
        value = v;
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
        value1 = v;
      },
      || completed1 = true,
    );

    assert_eq!(value1, 123);
    assert!(completed1);

    let mut value2 = 0;
    let mut completed2 = false;
    observable::of_option(None).subscribe_complete(
      |v| {
        value2 = v;
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
    observable::of_result(r).subscribe_all(
      |v| {
        value1 = v;
      },
      |_| {},
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
    observable::of(100).subscribe_complete(|v| value = v, || completed = true);

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

  #[test]
  fn repeat_three_times() {
    let mut hit_count = 0;
    let mut completed = false;
    observable::repeat(123, 5).subscribe_complete(
      |v| {
        hit_count += 1;
        assert_eq!(123, v);
      },
      || completed = true,
    );
    assert_eq!(5, hit_count);
    assert!(completed);
  }

  #[test]
  fn repeat_zero_times() {
    let mut hit_count = 0;
    let mut completed = false;
    observable::repeat(123, 0).subscribe_complete(
      |v| {
        hit_count += 1;
        assert_eq!(123, v);
      },
      || completed = true,
    );
    assert_eq!(0, hit_count);
    assert!(completed);
  }
}
