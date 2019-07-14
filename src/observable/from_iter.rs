use crate::prelude::*;

pub struct IterObservable<I>(I);

fn subscribe_impl<'a, Item: 'a, Err: 'a>(
  iter: impl IntoIterator<Item = Item> + Clone + 'a,
  next: impl Fn(&Item) -> OState<Err> + 'a,
  error: Option<impl Fn(&Err) + 'a>,
  complete: Option<impl Fn() + 'a>,
) -> Subscriber<'a, Item, Err> {
  Observable::new(move |subscriber: &mut Subscriber<'a, Item, Err>| {
    iter
      .clone()
      .into_iter()
      .take_while(|_| !subscriber.is_stopped())
      .for_each(|v| {
        subscriber.next(&v);
      });
    if !subscriber.is_stopped() {
      subscriber.complete();
    }
  })
  .subscribe_return_state(next, error, complete)
}

pub fn from_iter<Item>(
  iter: impl IntoIterator<Item = Item> + Clone,
) -> IterObservable<impl IntoIterator<Item = Item> + Clone> {
  IterObservable(iter)
}

impl<'a, I> ImplSubscribable<'a> for IterObservable<I>
where
  I: IntoIterator + Clone + 'a,
{
  type Item = I::Item;
  type Err = ();
  type Unsub = Subscriber<'a, I::Item, ()>;
  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsub {
    subscribe_impl(self.0, next, error, complete)
  }
}

impl<'a, I> ImplSubscribable<'a> for &'a IterObservable<I>
where
  I: IntoIterator + Clone,
{
  type Item = I::Item;
  type Err = ();
  type Unsub = Subscriber<'a, I::Item, ()>;
  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsub {
    subscribe_impl(self.0.clone(), next, error, complete)
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::cell::Cell;

  #[test]
  fn from_range() {
    let hit_count = Cell::new(0);
    let completed = Cell::new(false);
    observable::from_iter(0..100).subscribe_complete(
      |_| hit_count.set(hit_count.get() + 1),
      || completed.set(true),
    );

    assert_eq!(hit_count.get(), 100);
    assert_eq!(completed.get(), true);
  }

  #[test]
  fn from_vec() {
    let hit_count = Cell::new(0);
    let completed = Cell::new(false);
    observable::from_iter(vec![0; 100].iter()).subscribe_complete(
      |_| hit_count.set(hit_count.get() + 1),
      || completed.set(true),
    );

    assert_eq!(hit_count.get(), 100);
    assert_eq!(completed.get(), true);
  }

  #[test]
  fn fork() {
    use crate::ops::{Filter, Fork};
    observable::from_iter(vec![0; 100].iter())
      .fork()
      .filter(|_v| true)
      .fork()
      .subscribe(|_| {});
  }
}
