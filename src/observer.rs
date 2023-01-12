use crate::rc::{MutArc, MutRc, RcDeref, RcDerefMut};

/// An Observer is a consumer of values delivered by an Observable. One for each
/// type of notification delivered by the Observable: `next`, `error`,
/// and `complete`.
///
/// `Item` the type of the elements being emitted.
/// `Err`the type of the error may propagating.
pub trait Observer<Item, Err> {
  fn next(&mut self, value: Item);
  fn error(self, err: Err);
  fn complete(self);
  /// return if the observer is finished, that means the observer
  /// can emit value or not. false can, true not.
  ///
  /// When `error` or `complete` method called, the observer finished.
  /// Because `error` and `complete` method with move semantics, most of
  /// observers have no chance to call this method. But for some complicated
  /// observer, although not call itself `error` or `complete`, it finished
  /// because its child observer finished.
  fn is_finished(&self) -> bool;
}

pub struct BoxObserver<'a, Item, Err>(
  Box<dyn BoxObserverInner<Item, Err> + 'a>,
);
pub struct BoxObserverThreads<Item, Err>(
  Box<dyn BoxObserverInner<Item, Err> + Send>,
);

trait BoxObserverInner<Item, Err> {
  fn box_next(&mut self, value: Item);
  fn box_error(self: Box<Self>, err: Err);
  fn box_complete(self: Box<Self>);
  fn box_is_finished(&self) -> bool;
}

impl<Item, Err, T> BoxObserverInner<Item, Err> for T
where
  T: Observer<Item, Err>,
{
  #[inline]
  fn box_next(&mut self, value: Item) {
    self.next(value)
  }

  #[inline]
  fn box_error(self: Box<Self>, err: Err) {
    self.error(err)
  }

  #[inline]
  fn box_complete(self: Box<Self>) {
    self.complete()
  }

  #[inline]
  fn box_is_finished(&self) -> bool {
    self.is_finished()
  }
}

macro_rules! impl_observer_for_boxed {
  ($ty: ty $(, $lf:lifetime)? ) => {
    impl<$($lf,)? Item, Err> Observer<Item, Err> for $ty
    {
      #[inline]
      fn next(&mut self, value: Item) {
        self.0.box_next(value)
      }

      #[inline]
      fn error(self, err: Err) {
        self.0.box_error(err)
      }

      #[inline]
      fn complete(self) {
        self.0.box_complete()
      }

      #[inline]
      fn is_finished(&self) -> bool {
        self.0.box_is_finished()
      }
    }

  };
}

impl_observer_for_boxed!(BoxObserver<'a,Item,Err>, 'a);
impl_observer_for_boxed!(BoxObserverThreads<Item, Err>);

impl<'a, Item, Err> BoxObserver<'a, Item, Err> {
  #[inline]
  pub fn new(o: impl Observer<Item, Err> + 'a) -> Self {
    Self(Box::new(o))
  }
}

impl<Item, Err> BoxObserverThreads<Item, Err> {
  #[inline]
  pub fn new(o: impl Observer<Item, Err> + Send + 'static) -> Self {
    Self(Box::new(o))
  }
}

macro_rules! impl_rc_observer {
  ($rc: ident) => {
    impl<Item, Err, O> Observer<Item, Err> for $rc<Option<O>>
    where
      O: Observer<Item, Err>,
    {
      fn next(&mut self, value: Item) {
        if let Some(o) = &mut *self.rc_deref_mut() {
          o.next(value)
        }
      }

      fn error(self, err: Err) {
        if let Some(o) = self.rc_deref_mut().take() {
          o.error(err)
        }
      }

      fn complete(self) {
        if let Some(o) = self.rc_deref_mut().take() {
          o.complete()
        }
      }

      fn is_finished(&self) -> bool {
        self.rc_deref().as_ref().map_or(true, |o| o.is_finished())
      }
    }
  };
}

impl_rc_observer!(MutRc);
impl_rc_observer!(MutArc);
