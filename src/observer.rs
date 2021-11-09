/// An Observer is a consumer of values delivered by an Observable. One for each
/// type of notification delivered by the Observable: `next`, `error`,
/// and `complete`.
///
/// `Item` the type of the elements being emitted.
/// `Err`the type of the error may propagating.
pub trait Observer {
  type Item;
  type Err;
  fn next(&mut self, value: Self::Item);
  fn error(&mut self, err: Self::Err);
  fn complete(&mut self);
}

impl<Item, Err, T> Observer for Box<T>
where
  T: Observer<Item = Item, Err = Err> + ?Sized,
{
  type Item = Item;
  type Err = Err;
  fn next(&mut self, value: Item) {
    let s = &mut **self;
    s.next(value)
  }
  fn error(&mut self, err: Err) {
    let s = &mut **self;
    s.error(err);
  }
  fn complete(&mut self) {
    let s = &mut **self;
    s.complete();
  }
}
