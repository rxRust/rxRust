use crate::{impl_local_shared_both, prelude::*};

/// Creates an observable that will on subscription defer to another observable
/// that is supplied by a supplier-function which will be run once at each
/// subscription
///
/// ```rust
/// # use rxrust::prelude::*;
///
/// observable::defer(|| {
///   println!("Hi!");
///   observable::of("Hello!")
/// })
///   .subscribe(move |v| {
///     println!("{}", v);
///   });
/// // Prints: Hi!\nHello!\n
/// ```
pub fn defer<F, U>(observable_supplier: F) -> ObservableDeref<F>
where
  F: FnOnce() -> U,
{
  ObservableDeref(observable_supplier)
}

#[derive(Clone)]
pub struct ObservableDeref<F>(F);

impl<F, U> Observable for ObservableDeref<F>
where
  F: FnOnce() -> U,
  U: Observable,
{
  type Item = U::Item;
  type Err = U::Err;
}

impl_local_shared_both! {
 impl<F, U> ObservableDeref<F>;
 type Unsub = U::Unsub;
 macro method($self: ident, $observer: ident, $ctx: ident) {
  ($self.0)().actual_subscribe($observer)
 }
 where F: FnOnce()-> U, U: @ctx::Observable
}

#[cfg(test)]
mod test {
  use std::ops::Deref;
  use std::sync::{Arc, Mutex};

  use crate::prelude::*;
  use bencher::Bencher;

  #[test]
  fn no_results_before_deferred_subscribe() {
    let calls = Arc::new(Mutex::new(0));
    let sum = Arc::new(Mutex::new(0));
    let errs = Arc::new(Mutex::new(0));
    let completes = Arc::new(Mutex::new(0));

    let deferred = observable::defer(|| {
      *calls.lock().unwrap() += 1;
      observable::of(&2)
    })
    .into_shared();

    assert_eq!(calls.lock().unwrap().deref(), &0);

    for i in 1..4 {
      let sum_copy = Arc::clone(&sum);
      let errs_copy = Arc::clone(&errs);
      let completes_copy = Arc::clone(&completes);
      deferred.clone().subscribe_all(
        move |v| *sum_copy.lock().unwrap() += v,
        move |_| *errs_copy.lock().unwrap() += 1,
        move || *completes_copy.lock().unwrap() += 1,
      );
      assert_eq!(*calls.lock().unwrap(), i);
    }

    assert_eq!(*calls.lock().unwrap().deref(), 3);
    assert_eq!(*sum.lock().unwrap().deref(), 6);
    assert_eq!(*errs.lock().unwrap().deref(), 0);
    assert_eq!(*completes.lock().unwrap().deref(), 3);
  }

  #[test]
  fn support_fork() {
    let calls = Arc::new(Mutex::new(0));
    let o = observable::defer(|| {
      *calls.lock().unwrap() += 1;
      observable::of(10)
    });
    let sum1 = Arc::new(Mutex::new(0));
    let sum2 = Arc::new(Mutex::new(0));
    let c_sum1 = sum1.clone();
    let c_sum2 = sum2.clone();
    o.clone().subscribe(move |v| *sum1.lock().unwrap() += v);
    o.clone().subscribe(move |v| *sum2.lock().unwrap() += v);

    assert_eq!(*c_sum1.lock().unwrap(), 10);
    assert_eq!(*c_sum2.lock().unwrap(), 10);
    assert_eq!(*calls.lock().unwrap().deref(), 2);
  }

  #[test]
  fn fork_and_share() {
    let observable = observable::defer(observable::empty);
    observable.clone().into_shared().subscribe(|_: i32| {});
    observable.into_shared().subscribe(|_| {});

    let observable = observable::defer(observable::empty).into_shared();
    observable.clone().subscribe(|_: i32| {});
    observable.subscribe(|_| {});
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_deref);

  fn bench_deref(b: &mut Bencher) {
    b.iter(no_results_before_deferred_subscribe);
  }
}
