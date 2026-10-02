//! WindowCount operator implementation
//!
//! Splits the source into windows of a fixed number of items.

use std::marker::PhantomData;

use crate::{
  context::Context,
  observable::{CoreObservable, ObservableType},
  observer::Observer,
};

/// WindowCount operator: Emit a new window every `count` items
///
/// Each window is a `Subject` wrapped in the context, emitted when it opens;
/// the first window opens at subscribe. A `count` of zero behaves like one.
///
/// # Examples
///
/// ```rust
/// use std::{cell::RefCell, rc::Rc};
///
/// use rxrust::prelude::*;
///
/// let windows = Rc::new(RefCell::new(Vec::new()));
/// let sink = windows.clone();
/// Local::from_iter(vec![1, 2, 3])
///   .window_count(2)
///   .subscribe(move |w: Local<_>| {
///     let bucket = Rc::new(RefCell::new(Vec::new()));
///     sink.borrow_mut().push(bucket.clone());
///     w.subscribe(move |v| bucket.borrow_mut().push(v));
///   });
/// let seen: Vec<Vec<i32>> = windows
///   .borrow()
///   .iter()
///   .map(|b| b.borrow().clone())
///   .collect();
/// assert_eq!(seen, vec![vec![1, 2], vec![3]]);
/// ```
#[doc(alias = "windowCount")]
pub struct WindowCount<S, CtxMarker> {
  pub source: S,
  pub count: usize,
  _marker: PhantomData<fn() -> CtxMarker>,
}

impl<S: Clone, CtxMarker> Clone for WindowCount<S, CtxMarker> {
  fn clone(&self) -> Self {
    Self { source: self.source.clone(), count: self.count, _marker: PhantomData }
  }
}

impl<S, CtxMarker> WindowCount<S, CtxMarker> {
  /// Windows of `count` items over `source`.
  pub fn new(source: S, count: usize) -> Self {
    Self { source, count: count.max(1), _marker: PhantomData }
  }
}

impl<S, CtxMarker> ObservableType for WindowCount<S, CtxMarker>
where
  S: ObservableType,
  CtxMarker: Context,
{
  type Item<'m>
    = CtxMarker::With<CtxMarker::Inner>
  where
    Self: 'm;
  type Err = S::Err;
}

/// Observer that counts items into windows
pub struct WindowCountObserver<O, Sub, CtxMarker> {
  observer: O,
  current: Option<Sub>,
  count: usize,
  seen: usize,
  _marker: PhantomData<fn() -> CtxMarker>,
}

impl<O, CtxMarker> WindowCountObserver<O, CtxMarker::Inner, CtxMarker>
where
  CtxMarker: Context<Inner: Default + Clone>,
{
  fn open<Err>(&mut self)
  where
    O: Observer<CtxMarker::With<CtxMarker::Inner>, Err>,
  {
    if self.observer.is_closed() {
      return;
    }
    let subject = CtxMarker::Inner::default();
    self
      .observer
      .next(CtxMarker::lift(subject.clone()));
    self.current = Some(subject);
    self.seen = 0;
  }
}

impl<O, CtxMarker, Item, Err> Observer<Item, Err>
  for WindowCountObserver<O, CtxMarker::Inner, CtxMarker>
where
  CtxMarker: Context<Inner: Default + Clone + Observer<Item, Err>>,
  O: Observer<CtxMarker::With<CtxMarker::Inner>, Err>,
  Err: Clone,
{
  fn next(&mut self, value: Item) {
    if self.current.is_none() {
      self.open::<Err>();
    }
    if let Some(window) = self.current.as_mut() {
      window.next(value);
    }
    self.seen += 1;
    if self.seen >= self.count
      && let Some(window) = self.current.take()
    {
      window.complete();
    }
  }

  fn error(self, err: Err) {
    if let Some(window) = self.current {
      window.error(err.clone());
    }
    self.observer.error(err);
  }

  fn complete(self) {
    if let Some(window) = self.current {
      window.complete();
    }
    self.observer.complete();
  }

  fn is_closed(&self) -> bool { self.observer.is_closed() }
}

impl<S, CtxMarker, C> CoreObservable<C> for WindowCount<S, CtxMarker>
where
  C: Context,
  CtxMarker:
    Context<Inner: Default + Clone + for<'a> Observer<<S as ObservableType>::Item<'a>, S::Err>>,
  C::Inner: Observer<CtxMarker::With<CtxMarker::Inner>, S::Err>,
  S: CoreObservable<C::With<WindowCountObserver<C::Inner, CtxMarker::Inner, CtxMarker>>>,
  S::Err: Clone,
{
  type Unsub = S::Unsub;

  fn subscribe(self, context: C) -> Self::Unsub {
    let WindowCount { source, count, .. } = self;
    let wrapped = context.transform(|observer| {
      let mut o =
        WindowCountObserver { observer, current: None, count, seen: 0, _marker: PhantomData };
      o.open::<S::Err>();
      o
    });
    source.subscribe(wrapped)
  }
}
