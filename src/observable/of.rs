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
    $crate::observable::create(|mut s| {
      $(
        s.next($item);
      )*
      s.complete();
    })
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
pub fn of<Item>(v: Item) -> ObservableBase<OfEmitter<Item>> {
  ObservableBase::new(OfEmitter(v))
}

#[derive(Clone)]
pub struct OfEmitter<Item>(pub(crate) Item);

impl<Item> Emitter for OfEmitter<Item> {
  type Item = Item;
  type Err = ();
}

impl<'a, Item> LocalEmitter<'a> for OfEmitter<Item> {
  type Unsub = SingleSubscription;

  fn emit<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    observer.next(self.0);
    observer.complete();
    SingleSubscription::default()
  }
}

impl<Item> SharedEmitter for OfEmitter<Item> {
  type Unsub = SingleSubscription;

  fn emit<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
  {
    observer.next(self.0);
    observer.complete();
    SingleSubscription::default()
  }
}

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
///   .subscribe_err(|v: &i32| {}, |e| {println!("Error:  {},", e)});
/// ```
pub fn of_result<Item, Err>(
  r: Result<Item, Err>,
) -> ObservableBase<ResultEmitter<Item, Err>> {
  ObservableBase::new(ResultEmitter(r))
}

#[derive(Clone)]
pub struct ResultEmitter<Item, Err>(pub(crate) Result<Item, Err>);

impl<Item, Err> Emitter for ResultEmitter<Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<'a, Item, Err> LocalEmitter<'a> for ResultEmitter<Item, Err> {
  type Unsub = SingleSubscription;

  fn emit<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    match self.0 {
      Ok(v) => observer.next(v),
      Err(e) => observer.error(e),
    };
    observer.complete();
    SingleSubscription::default()
  }
}

impl<Item, Err> SharedEmitter for ResultEmitter<Item, Err> {
  type Unsub = SingleSubscription;

  fn emit<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
  {
    match self.0 {
      Ok(v) => observer.next(v),
      Err(e) => observer.error(e),
    };
    observer.complete();
    SingleSubscription::default()
  }
}

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
pub fn of_option<Item>(o: Option<Item>) -> ObservableBase<OptionEmitter<Item>> {
  ObservableBase::new(OptionEmitter(o))
}

#[derive(Clone)]
pub struct OptionEmitter<Item>(pub(crate) Option<Item>);

impl<Item> Emitter for OptionEmitter<Item> {
  type Item = Item;
  type Err = ();
}

impl<'a, Item> LocalEmitter<'a> for OptionEmitter<Item> {
  type Unsub = SingleSubscription;

  fn emit<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    if let Some(v) = self.0 {
      observer.next(v)
    }
    observer.complete();
    SingleSubscription::default()
  }
}

impl<Item> SharedEmitter for OptionEmitter<Item> {
  type Unsub = SingleSubscription;

  fn emit<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
  {
    if let Some(v) = self.0 {
      observer.next(v)
    }
    observer.complete();
    SingleSubscription::default()
  }
}

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
pub fn of_fn<F, Item>(f: F) -> ObservableBase<CallableEmitter<F>>
where
  F: FnOnce() -> Item,
{
  ObservableBase::new(CallableEmitter(f))
}

#[derive(Clone)]
pub struct CallableEmitter<F>(F);

impl<Item, F> Emitter for CallableEmitter<F>
where
  F: FnOnce() -> Item,
{
  type Item = Item;
  type Err = ();
}

impl<'a, Item, F> LocalEmitter<'a> for CallableEmitter<F>
where
  F: FnOnce() -> Item,
{
  type Unsub = SingleSubscription;

  fn emit<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    observer.next((self.0)());
    observer.complete();
    SingleSubscription::default()
  }
}

impl<Item, F> SharedEmitter for CallableEmitter<F>
where
  F: FnOnce() -> Item,
{
  type Unsub = SingleSubscription;

  fn emit<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
  {
    observer.next((self.0)());
    observer.complete();
    SingleSubscription::default()
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn from_fn() {
    let mut value = 0;
    let mut completed = false;
    let callable = || 123;
    observable::of_fn(callable).subscribe_complete(
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
  fn of() {
    let mut value = 0;
    let mut completed = false;
    observable::of(100).subscribe_complete(|v| value = v, || completed = true);

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
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_of);

  fn bench_of(b: &mut bencher::Bencher) { b.iter(of); }
}
