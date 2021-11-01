use crate::prelude::*;

pub trait Emitter {
  type Item;
  type Err;
}

// todo: can we merge local & shared emitter
// and generic `O` type should be associated type?
pub trait LocalEmitter<'a>: Emitter {
  type Unsub: SubscriptionLike;
  fn emit<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a;
}

#[derive(Clone)]
pub struct ObservableBase<Emit>(Emit);

impl<Emit> ObservableBase<Emit> {
  pub fn new(emitter: Emit) -> Self { ObservableBase(emitter) }
}

impl<Emit> Observable for ObservableBase<Emit>
where
  Emit: Emitter,
{
  type Item = Emit::Item;
  type Err = Emit::Err;
}

impl<'a, Emit> LocalObservable<'a> for ObservableBase<Emit>
where
  Emit: LocalEmitter<'a>,
{
  type Unsub = Emit::Unsub;

  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: observer::Observer<Item = Emit::Item, Err = Emit::Err> + 'a,
  {
    self.0.emit(observer)
  }
}

impl<Emit> SharedObservable for ObservableBase<Emit>
where
  Emit: SharedEmitter,
{
  type Unsub = Emit::Unsub;

  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Emit::Item, Err = Emit::Err> + Send + Sync + 'static,
  {
    self.0.emit(observer)
  }
}
