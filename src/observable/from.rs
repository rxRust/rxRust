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

/// Creates an observable that emits no items and terminates with an error.
pub fn throw<O, U, Item, Err>(
  v: Err,
) -> Observable<impl FnOnce(Subscriber<O, U>) + Clone>
where
  O: Observer<Item, Err>,
  U: SubscriptionLike,
  Err: Clone,
  Item: Clone,
{
  Observable::new(move |mut subscriber| {
    subscriber.error(&v);
  })
}

/// Creates an observable that emits result's value provided by [`Result`] or the error.
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
/// observable::of_result(Err("An error"))
///   .subscribe(|v| {println!("{},", v)});
/// 
/// // result: nothing printed
/// ```
/// 
pub fn of_result<O, U, Item,Err>(
  r: Result<Item, Err>,
) -> Observable<impl FnOnce(Subscriber<O, U>) + Clone>
where
  O: Observer<Item, Err>,
  U: SubscriptionLike,
  Item: Clone,
  Err: Clone,
{
  Observable::new(move |mut subscriber| {
    match &r
    {
      Ok(v) => subscriber.next(v),
      Err(e) => subscriber.error(e),
    };
    subscriber.complete();
  })
}

/// Creates an observable that potentially emits a single value from [`Option`].
/// 
/// Emits the value if is there, and completes immediatelly after. When given option
/// has not value, completes immediatelly. Never emitts an error.
/// 
/// # Examples 
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::of_result(Some(1234))
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
    match &o
    {
      Some(v) => subscriber.next(v),
      None => ()
    };
    subscriber.complete();
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
///   .subscribe(|v| {println!("{},", v)});
///
/// // Result: no thing printed
/// ```
/// 
pub fn empty<O, U, Item>() -> Observable<impl FnOnce(Subscriber<O, U>) + Clone>
where
  O: Observer<Item, ()>,
  U: SubscriptionLike,
{
  Observable::new(move |mut subscriber: Subscriber<O, U>| {
    subscriber.complete();
  })
}

/// Creates an observable that never emitts any value and never completes.
///
/// Neither emitts an error.
///
pub fn never<O, U, Item>() -> Observable<impl FnOnce(Subscriber<O, U>) + Clone>
where
  O: Observer<Item, ()>,
  U: SubscriptionLike,
{
  Observable::new(move |_subscriber: Subscriber<O, U>| {
    loop {
      // will not complete
    }
  })
}

/// Creates an observable that emits the return value of a function-like callable.
/// 
/// Never emits an error
/// 
/// # Examples
/// 
/// ```
/// use rxrust::prelude::*;
///
/// observable::start(|| {1234})
///   .subscribe(|v| {println!("{},", v)});
/// ```
/// 
pub fn start<O, U, Callable, Item>(
  func: Callable,
) -> Observable<impl FnOnce(Subscriber<O, U>) + Clone>
where
  Callable: FnOnce() -> Item,
  O: Observer<Item, ()>,
  U: SubscriptionLike,
  Item: Clone,
{
  // because of Rust zero-cost abstraction
  // we can compose from what we have already
  // without fearing of introducing unwanted overhead
  of(func())
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

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
  fn empty() {
    let mut hits = 0;
    let mut completed = false;
    observable::empty()
      .subscribe_complete(|_: &()| hits += 1, || completed = true);

    assert_eq!(hits, 0);
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
