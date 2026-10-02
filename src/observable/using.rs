//! Using: tie a resource's lifetime to a subscription.

use std::marker::PhantomData;

use crate::{
  context::{Context, RcDerefMut},
  observable::{CoreObservable, ObservableType},
  observer::Observer,
  subscription::Subscription,
};

/// Creates a resource per subscription, builds the observable from it, and
/// drops the resource when the subscription terminates or is unsubscribed.
///
/// # Examples
///
/// ```rust
/// use std::{cell::RefCell, rc::Rc};
///
/// use rxrust::prelude::*;
///
/// let dropped = Rc::new(RefCell::new(false));
/// struct Guard(Rc<RefCell<bool>>);
/// impl Drop for Guard {
///   fn drop(&mut self) { *self.0.borrow_mut() = true; }
/// }
///
/// let flag = dropped.clone();
/// Local::using(move || Guard(flag.clone()), |_guard| Local::from_iter(vec![1, 2]))
///   .subscribe(|v| println!("{}", v));
/// assert!(*dropped.borrow());
/// ```
pub struct Using<RF, OF, Res, OutCtx> {
  pub resource_factory: RF,
  pub observable_factory: OF,
  _marker: PhantomData<fn() -> (Res, OutCtx)>,
}

impl<RF, OF, Res, OutCtx> Using<RF, OF, Res, OutCtx> {
  /// Pairs a resource factory with an observable factory.
  pub fn new(resource_factory: RF, observable_factory: OF) -> Self {
    Self { resource_factory, observable_factory, _marker: PhantomData }
  }
}

impl<RF, OF, Res, OutCtx> ObservableType for Using<RF, OF, Res, OutCtx>
where
  OutCtx: Context<Inner: ObservableType>,
{
  type Item<'a>
    = <OutCtx::Inner as ObservableType>::Item<'a>
  where
    Self: 'a;
  type Err = <OutCtx::Inner as ObservableType>::Err;
}

/// Observer that releases the resource on a terminal event
pub struct UsingObserver<O, H> {
  observer: O,
  holder: H,
}

impl<O, H, Res, Item, Err> Observer<Item, Err> for UsingObserver<O, H>
where
  O: Observer<Item, Err>,
  H: RcDerefMut<Target = Option<Res>>,
{
  fn next(&mut self, value: Item) { self.observer.next(value); }

  fn error(self, err: Err) {
    self.holder.rc_deref_mut().take();
    self.observer.error(err);
  }

  fn complete(self) {
    self.holder.rc_deref_mut().take();
    self.observer.complete();
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

/// Subscription that releases the resource when unsubscribed
pub struct UsingSubscription<U, H> {
  inner: U,
  holder: H,
}

impl<U, H, Res> Subscription for UsingSubscription<U, H>
where
  U: Subscription,
  H: RcDerefMut<Target = Option<Res>>,
{
  fn unsubscribe(self) {
    self.inner.unsubscribe();
    self.holder.rc_deref_mut().take();
  }

  fn is_closed(&self) -> bool { self.inner.is_closed() || self.holder.rc_deref().is_none() }
}

type Holder<C, Res> = <C as Context>::RcMut<Option<Res>>;
type InnerCtx<C, Res> = <C as Context>::With<UsingObserver<<C as Context>::Inner, Holder<C, Res>>>;

impl<RF, OF, Res, OutCtx, C> CoreObservable<C> for Using<RF, OF, Res, OutCtx>
where
  C: Context,
  RF: FnOnce() -> Res,
  OF: FnOnce(&Res) -> OutCtx,
  OutCtx: Context<Inner: CoreObservable<InnerCtx<C, Res>>>,
{
  type Unsub =
    UsingSubscription<<OutCtx::Inner as CoreObservable<InnerCtx<C, Res>>>::Unsub, Holder<C, Res>>;

  fn subscribe(self, context: C) -> Self::Unsub {
    let resource = (self.resource_factory)();
    let source = (self.observable_factory)(&resource).into_inner();
    let holder: Holder<C, Res> = C::RcMut::from(Some(resource));
    let holder_for_observer = holder.clone();
    let wrapped =
      context.transform(|observer| UsingObserver { observer, holder: holder_for_observer });
    let inner = source.subscribe(wrapped);
    UsingSubscription { inner, holder }
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, convert::Infallible, rc::Rc};

  use crate::prelude::*;

  struct Guard(Rc<RefCell<bool>>);
  impl Drop for Guard {
    fn drop(&mut self) { *self.0.borrow_mut() = true; }
  }

  #[rxrust_macro::test]
  fn test_using_drops_resource_on_complete() {
    let dropped = Rc::new(RefCell::new(false));
    let result = Rc::new(RefCell::new(Vec::new()));
    let flag = dropped.clone();
    let result_c = result.clone();

    Local::using(move || Guard(flag.clone()), |_g| Local::from_iter(vec![1, 2]))
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![1, 2]);
    assert!(*dropped.borrow());
  }

  #[rxrust_macro::test]
  fn test_using_drops_resource_on_unsubscribe() {
    let dropped = Rc::new(RefCell::new(false));
    let flag = dropped.clone();
    let source = Local::subject::<i32, Infallible>();
    let source_c = source.clone();

    let sub =
      Local::using(move || Guard(flag.clone()), move |_g| source_c.clone()).subscribe(|_| {});
    assert!(!*dropped.borrow());
    assert_eq!(source.inner.subscriber_count(), 1);

    sub.unsubscribe();
    assert!(*dropped.borrow());
    assert_eq!(source.inner.subscriber_count(), 0);
  }

  #[rxrust_macro::test]
  fn test_using_resource_available_to_factory() {
    let result = Rc::new(RefCell::new(Vec::new()));
    let result_c = result.clone();

    Local::using(|| 21, |base| Local::of(*base * 2))
      .subscribe(move |v| result_c.borrow_mut().push(v));

    assert_eq!(*result.borrow(), vec![42]);
  }
}
