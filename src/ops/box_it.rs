use crate::{
  observer::{BoxObserver, BoxObserverThreads},
  prelude::*,
};

pub trait BoxIt<O>: Sized {
  /// box an observable to a safety object and convert it to a simple type
  /// `BoxOp`, which only care `Item` and `Err` Observable emitted.
  ///
  /// # Example
  /// ```
  /// use rxrust::{prelude::*, ops::box_it::BoxOp};
  ///
  /// let mut boxed: BoxOp<'_, i32, _> = observable::of(1)
  ///   .map(|v| v).box_it();
  ///
  /// // BoxOp can box any observable type
  /// boxed = observable::empty().box_it();
  ///
  /// boxed.subscribe(|_| {});
  /// ```
  fn box_it(self) -> O;
}

pub struct BoxOp<'a, Item, Err>(Box<dyn BoxObservable<'a, Item, Err> + 'a>);
pub struct CloneableBoxOp<'a, Item, Err>(
  Box<dyn CloneableBox<'a, Item, Err> + 'a>,
);
pub struct BoxOpThreads<Item, Err>(
  Box<dyn BoxObservableThreads<Item, Err> + Send>,
);
pub struct CloneableBoxOpThreads<Item, Err>(
  Box<dyn CloneableBoxThreads<Item, Err> + Send>,
);

trait BoxObservable<'a, Item, Err> {
  fn box_subscribe(
    self: Box<Self>,
    observer: BoxObserver<'a, Item, Err>,
  ) -> BoxSubscription;
}

trait BoxObservableThreads<Item, Err> {
  fn box_subscribe(
    self: Box<Self>,
    observer: BoxObserverThreads<Item, Err>,
  ) -> BoxSubscriptionThreads;
}

trait CloneableBox<'a, Item, Err>: BoxObservable<'a, Item, Err> {
  fn box_clone(&self) -> Box<dyn CloneableBox<'a, Item, Err> + 'a>;
}

trait CloneableBoxThreads<Item, Err>: BoxObservableThreads<Item, Err> {
  fn box_clone(&self) -> Box<dyn CloneableBoxThreads<Item, Err> + Send>;
}

impl<'a, Item, Err, T> BoxObservable<'a, Item, Err> for T
where
  T: Observable<Item, Err, BoxObserver<'a, Item, Err>>,
  T::Unsub: 'a,
{
  fn box_subscribe(
    self: Box<Self>,
    observer: BoxObserver<'a, Item, Err>,
  ) -> BoxSubscription {
    let u = self.actual_subscribe(observer);
    BoxSubscription::new(u)
  }
}

impl<Item, Err, T> BoxObservableThreads<Item, Err> for T
where
  T: Observable<Item, Err, BoxObserverThreads<Item, Err>>,
  T::Unsub: Send + 'static,
{
  fn box_subscribe(
    self: Box<Self>,
    observer: BoxObserverThreads<Item, Err>,
  ) -> BoxSubscriptionThreads {
    let u = self.actual_subscribe(observer);
    BoxSubscriptionThreads::new(u)
  }
}

macro_rules! impl_observable_for_box {
  (
    $ty: ty,
    $box_observer: ident,
    $subscription:ty
    $(,$lf:lifetime)?
    $(,$send:ident)?
  ) => {
    impl<$($lf,)? Item, Err, O> Observable<Item, Err, O>
      for $ty
    where
      O: Observer<Item, Err> $(+$lf)? $(+ $send +'static)?,
    {
      type Unsub = $subscription;

      #[inline]
      fn actual_subscribe(self, observer: O) -> Self::Unsub {
        self.0.box_subscribe($box_observer::new(observer))
      }
    }

    impl<$($lf,)? Item, Err> ObservableExt<Item, Err> for $ty {
    }
  };
}

impl_observable_for_box!(BoxOp<'a, Item,Err>, BoxObserver, BoxSubscription<'a>, 'a);
impl_observable_for_box!(BoxOpThreads<Item,Err>, BoxObserverThreads, BoxSubscriptionThreads, Send);
impl_observable_for_box!(CloneableBoxOp<'a,Item,Err>, BoxObserver, BoxSubscription<'a>, 'a);
impl_observable_for_box!(CloneableBoxOpThreads<Item,Err>, BoxObserverThreads, BoxSubscriptionThreads, Send);

macro_rules! impl_box_it {
  ($($lf:lifetime,)? $name: ident, $($bounds: tt)*) => {
    impl<$($lf,)? Item, Err, O> BoxIt<$name<$($lf,)? Item, Err>> for O
    where
      O: $($bounds)*,
    {
      #[inline]
      fn box_it(self) -> $name<$($lf,)? Item, Err> {
        $name(Box::new(self))
      }
    }
  };
}

impl_box_it!('a, BoxOp, BoxObservable<'a, Item,Err> +'a);
impl_box_it!('a, CloneableBoxOp, CloneableBox<'a, Item,Err> +'a);
impl_box_it!(BoxOpThreads, BoxObservableThreads<Item, Err> + Send + 'static);
impl_box_it!(CloneableBoxOpThreads, CloneableBoxThreads<Item, Err> + Send + 'static);

impl<'a, Item: 'a, Err: 'a> Clone for CloneableBoxOp<'a, Item, Err> {
  #[inline]
  fn clone(&self) -> Self {
    Self(self.0.box_clone())
  }
}

impl<Item, Err> Clone for CloneableBoxOpThreads<Item, Err> {
  #[inline]
  fn clone(&self) -> Self {
    Self(self.0.box_clone())
  }
}

impl<'a, Item, Err, T> CloneableBox<'a, Item, Err> for T
where
  T: BoxObservable<'a, Item, Err> + Clone + 'a,
{
  #[inline]
  fn box_clone(&self) -> Box<dyn CloneableBox<'a, Item, Err> + 'a> {
    Box::new(self.clone())
  }
}

impl<Item, Err, T> CloneableBoxThreads<Item, Err> for T
where
  T: BoxObservableThreads<Item, Err> + Clone + Send + 'static,
{
  #[inline]
  fn box_clone(&self) -> Box<dyn CloneableBoxThreads<Item, Err> + Send> {
    Box::new(self.clone())
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
  use super::*;
  use bencher::Bencher;

  #[test]
  fn box_observable() {
    let mut test = 0;
    let mut boxed: BoxOp<'_, i32, _> = observable::of(100).box_it();
    boxed.subscribe(|v| test = v);

    boxed = observable::empty().box_it();
    boxed.subscribe(|_| unreachable!());
    assert_eq!(test, 100);
  }

  #[test]
  fn shared_box_observable() {
    let mut boxed: BoxOpThreads<i32, _> = observable::of(100).box_it();
    boxed.subscribe(|_| {});

    boxed = observable::empty().box_it();
    boxed.subscribe(|_| unreachable!());
  }

  #[test]
  fn box_clone() {
    let boxed: CloneableBoxOp<_, _> = observable::of(100).box_it();
    boxed.clone().subscribe(|_| {});
  }

  #[test]
  fn bench() {
    do_bench();
  }

  benchmark_group!(do_bench, bench_box_clone);

  fn bench_box_clone(b: &mut Bencher) {
    b.iter(box_clone);
  }
}
