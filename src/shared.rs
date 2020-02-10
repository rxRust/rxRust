use crate::prelude::*;

#[derive(Clone)]
pub struct Shared<R: Send + Sync + 'static>(pub(crate) R);

impl<R> Fork for Shared<R>
where
  R: Send + Sync + 'static + Clone,
{
  type Output = Self;
  #[inline]
  fn fork(&self) -> Self::Output { self.clone() }
}

pub trait SharedObservable {
  type Item;
  type Err;
  fn shared_actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> SharedSubscription;

  #[inline]
  fn shared(self) -> Shared<Self>
  where
    Self: Sized + Send + Sync + 'static,
  {
    Shared(self)
  }
}

pub trait SharedEmitter {
  type Item;
  type Err;
  fn shared_emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
  where
    O: Observer<Self::Item, Self::Err> + Send + Sync + 'static;
}

impl<S> SharedObservable for Shared<S>
where
  S: SharedObservable + Send + Sync + 'static,
{
  type Item = S::Item;
  type Err = S::Err;

  #[inline]
  fn shared_actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> SharedSubscription {
    self.0.shared_actual_subscribe(subscriber)
  }
}

pub(crate) macro auto_impl_shared_observable($t: ty,
  <$($generics: ident  $(: $bound: tt)?),*>) {
    impl<$($generics $(:$bound)?),*> SharedEmitter for $t
  where
    Self: SharedObservable<'static> + Send + Sync + 'static
  {
      type Item = <Self as Observable>::Item;
      type Err = <Self as Observable>::Err;

      #[inline(always)]
      fn shared_actual_subscribe<
        O: Observer<Self::Item, Self::Err> + Sync + Send + 'static>(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> SharedSubscription {
      self.actual_subscribe(subscriber)
  }

    }
  }

pub(crate) macro auto_impl_shared_emitter($t: ty
  $(, <$($generics: ident  $(: $bound: tt)?),*>)?) {
  impl$(<$($generics $(:$bound)?),*>)? SharedEmitter for $t
  where
    Self: Emitter<'static> + Send + Sync + 'static
  {
    type Item = <Self as Emitter<'static>>::Item;
    type Err = <Self as Emitter<'static>>::Err;
    #[inline(always)]
    fn shared_emit<O>(self, subscriber: Subscriber<O, SharedSubscription>)
    where
      O: Observer<Self::Item, Self::Err> + Send + Sync + 'static,
    {
      self.emit(subscriber)
    }
  }
}
