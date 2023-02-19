use std::convert::Infallible;

use crate::prelude::*;

/// Creates an observable producing a multiple values.
///
/// Completes immediately after emitting the values given. Never emits an error.
///
/// # Arguments
///
/// * `v` - A value to emits.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
/// use rxrust::of_sequence;
///
/// of_sequence!(1, 2, 3)
///   .subscribe(|v| {println!("{},", v)});
///
/// // print log:
/// // 1
/// // 2
/// // 3
/// ```
#[macro_export]
macro_rules! of_sequence {
    ( $( $item:expr ),* ) => {
  {
    $crate::observable::from_iter([$($item),*])
  }
}
}

/// Creates an observable producing a single value.
///
/// Completes immediately after emitting the value given. Never emits an error.
///
/// # Arguments
///
/// * `v` - A value to emits.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::of(123)
///   .subscribe(|v| {println!("{},", v)});
/// ```
#[inline]
pub fn of<Item>(v: Item) -> OfObservable<Item> {
  OfObservable(v)
}

#[derive(Clone)]
pub struct OfObservable<Item>(pub(crate) Item);

impl<Item, O> Observable<Item, Infallible, O> for OfObservable<Item>
where
  O: Observer<Item, Infallible>,
{
  type Unsub = ();

  fn actual_subscribe(self, mut observer: O) -> Self::Unsub {
    observer.next(self.0);
    observer.complete();
  }
}

impl<Item> ObservableExt<Item, Infallible> for OfObservable<Item> {}
/// Creates an observable that emits value or the error from a [`Result`] given.
///
/// Completes immediately after.
///
/// # Arguments
///
/// * `r` - A [`Result`] argument to take a value, or an error to emits from.
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
///   .on_error(|e| println!("Error:  {},", e))
///   .subscribe(|v: &i32| {});
/// ```
pub fn of_result<Item, Err>(
  r: Result<Item, Err>,
) -> ResultObservable<Item, Err> {
  ResultObservable(r)
}

#[derive(Clone)]
pub struct ResultObservable<Item, Err>(pub(crate) Result<Item, Err>);

impl<Item, Err, O> Observable<Item, Err, O> for ResultObservable<Item, Err>
where
  O: Observer<Item, Err>,
{
  type Unsub = ();

  fn actual_subscribe(self, mut observer: O) -> Self::Unsub {
    match self.0 {
      Ok(v) => {
        observer.next(v);
        observer.complete();
      }
      Err(e) => observer.error(e),
    };
  }
}

impl<Item, Err> ObservableExt<Item, Err> for ResultObservable<Item, Err> {}

/// Creates an observable that potentially emits a single value from [`Option`].
///
/// Emits the value if is there, and completes immediately after. When the
/// given option has not value, completes immediately. Never emits an error.
///
/// # Arguments
///
/// * `o` - An optional used to take a value to emits from.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::of_option(Some(1234))
///   .subscribe(|v| {println!("{},", v)});
/// ```
pub fn of_option<Item>(o: Option<Item>) -> OptionObservable<Item> {
  OptionObservable(o)
}

#[derive(Clone)]
pub struct OptionObservable<Item>(pub(crate) Option<Item>);

impl<Item, O> Observable<Item, Infallible, O> for OptionObservable<Item>
where
  O: Observer<Item, Infallible>,
{
  type Unsub = ();

  fn actual_subscribe(self, mut observer: O) -> Self::Unsub {
    if let Some(v) = self.0 {
      observer.next(v)
    }
    observer.complete();
  }
}

impl<Item> ObservableExt<Item, Infallible> for OptionObservable<Item> {}

/// Creates an observable that emits the return value of a callable.
///
/// Never emits an error.
///
/// # Arguments
///
/// * `f` - A function that will be called to obtain its return value to emits.
///
/// # Examples
///
/// ```
/// use rxrust::prelude::*;
///
/// observable::of_fn(|| {1234})
///   .subscribe(|v| {println!("{},", v)});
/// ```
pub fn of_fn<F, Item>(f: F) -> CallableObservable<F>
where
  F: FnOnce() -> Item,
{
  CallableObservable(f)
}

#[derive(Clone)]
pub struct CallableObservable<F>(pub(crate) F);

impl<Item, F, O> Observable<Item, Infallible, O> for CallableObservable<F>
where
  F: FnOnce() -> Item,
  O: Observer<Item, Infallible>,
{
  type Unsub = ();

  fn actual_subscribe(self, mut observer: O) -> Self::Unsub {
    observer.next((self.0)());
    observer.complete();
  }
}

impl<Item, F> ObservableExt<Item, Infallible> for CallableObservable<F> where
  F: FnOnce() -> Item
{
}
#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn from_fn() {
    let mut value = 0;
    let mut completed = false;
    let callable = || 123;
    observable::of_fn(callable)
      .on_complete(|| completed = true)
      .subscribe(|v| {
        value = v;
      });

    assert_eq!(value, 123);
    assert!(completed);
  }

  #[test]
  fn of_option() {
    let mut value1 = 0;
    let mut completed1 = false;
    observable::of_option(Some(123))
      .on_complete(|| completed1 = true)
      .subscribe(|v| {
        value1 = v;
      });

    assert_eq!(value1, 123);
    assert!(completed1);

    let mut value2 = 0;
    let mut completed2 = false;
    observable::of_option(None)
      .on_complete(|| completed2 = true)
      .subscribe(|v| {
        value2 = v;
      });

    assert_eq!(value2, 0);
    assert!(completed2);
  }

  #[test]
  fn of_result() {
    let mut value1 = 0;
    let mut completed1 = false;
    let r: Result<i32, &str> = Ok(123);
    observable::of_result(r)
      .on_complete(|| completed1 = true)
      .on_error(|_| {})
      .subscribe(|v| {
        value1 = v;
      });

    assert_eq!(value1, 123);
    assert!(completed1);

    let mut value2 = 0;
    let mut error_reported = false;
    let r: Result<i32, &str> = Err("error");
    observable::of_result(r)
      .on_error(|_| error_reported = true)
      .subscribe(|_| value2 = 123);

    assert_eq!(value2, 0);
    assert!(error_reported);
  }

  #[test]
  fn of() {
    let mut value = 0;
    let mut completed = false;
    observable::of(100)
      .on_complete(|| completed = true)
      .subscribe(|v| value = v);

    assert_eq!(value, 100);
    assert!(completed);
  }

  #[test]
  fn of_macros() {
    let mut value = 0;
    of_sequence!(1, 2, 3).subscribe(|v| value += v);

    assert_eq!(value, 6);
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_of);

  fn bench_of(b: &mut bencher::Bencher) {
    b.iter(of);
  }
}
