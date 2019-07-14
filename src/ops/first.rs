use crate::{
  ops::{take::TakeOp, Take},
  prelude::*,
};
use std::{cell::RefCell, rc::Rc};

/// emit only the first item emitted by an Observable
pub trait First {
  fn first(self) -> TakeOp<Self>
  where
    Self: Sized + Take,
  {
    self.take(1)
  }
}

impl<'a, O> First for O where O: ImplSubscribable<'a> {}

/// emit only the first item (or a default item) emitted by an Observable
pub trait FirstOr<'a> {
  fn first_or(self, default: Self::Item) -> FirstOrOp<TakeOp<Self>, Self::Item>
  where
    Self: ImplSubscribable<'a>,
  {
    FirstOrOp {
      source: self.first(),
      default: Some(default),
    }
  }
}

impl<'a, O> FirstOr<'a> for O where O: ImplSubscribable<'a> {}

pub struct FirstOrOp<S, V> {
  source: S,
  default: Option<V>,
}

fn subscribe_source<'a, S, V>(
  source: S,
  default: Option<V>,
  next: impl Fn(&S::Item) -> OState<S::Err> + 'a,
  error: Option<impl Fn(&S::Err) + 'a>,
  complete: Option<impl Fn() + 'a>,
) -> S::Unsub
where
  V: 'a,
  S: ImplSubscribable<'a, Item = V>,
{
  let next = Rc::new(next);
  let c_next = next.clone();
  let default = Rc::new(RefCell::new(default));
  let c_default = default.clone();
  source.subscribe_return_state(
    move |v| {
      c_default.borrow_mut().take();
      c_next(v)
    },
    error,
    Some(move || {
      let default = default.borrow_mut().take();
      if let Some(d) = default {
        next(&d);
      }
      if let Some(ref comp) = complete {
        comp();
      }
    }),
  )
}

impl<'a, S, T> ImplSubscribable<'a> for FirstOrOp<S, T>
where
  T: 'a,
  S: ImplSubscribable<'a, Item = T>,
{
  type Item = S::Item;
  type Err = S::Err;
  type Unsub = S::Unsub;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsub {
    subscribe_source(self.source, self.default, next, error, complete)
  }
}

impl<'a, S, T> ImplSubscribable<'a> for &'a FirstOrOp<S, T>
where
  T: Clone + 'a,
  &'a S: ImplSubscribable<'a, Item = T>,
{
  type Err = <&'a S as ImplSubscribable<'a>>::Err;
  type Item = <&'a S as ImplSubscribable<'a>>::Item;
  type Unsub = <&'a S as ImplSubscribable<'a>>::Unsub;

  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsub {
    subscribe_source(&self.source, self.default.clone(), next, error, complete)
  }
}

#[cfg(test)]
mod test {
  use super::{First, FirstOr};
  use crate::prelude::*;
  use std::cell::Cell;

  #[test]
  fn first() {
    let completed = Cell::new(false);
    let next_count = Cell::new(0);

    let numbers = Subject::<'_, _, ()>::new();
    numbers.clone().first().subscribe_complete(
      |_| next_count.set(next_count.get() + 1),
      || completed.set(true),
    );

    (0..2).for_each(|v| {
      numbers.next(&v);
    });

    assert_eq!(completed.get(), true);
    assert_eq!(next_count.get(), 1);
  }

  #[test]
  fn first_or() {
    let completed = Cell::new(false);
    let next_count = Cell::new(0);
    let v = Cell::new(0);

    let mut numbers = Subject::<'_, i32, ()>::new();
    numbers.clone().first_or(100).subscribe_complete(
      |_| next_count.set(next_count.get() + 1),
      || completed.set(true),
    );

    // normal pass value
    (0..2).for_each(|v| {
      numbers.next(&v);
    });
    assert_eq!(next_count.get(), 1);
    assert_eq!(completed.get(), true);

    completed.set(false);
    numbers
      .clone()
      .first_or(100)
      .subscribe_complete(|value| v.set(*value), || completed.set(true));

    numbers.complete();
    assert_eq!(completed.get(), true);
    assert_eq!(v.get(), 100);
  }

  #[test]
  fn first_support_fork() {
    use crate::ops::{First, Fork};
    let value = Cell::new(0);
    let o = observable::from_iter(1..100).first();
    let o1 = o.fork().first();
    let o2 = o.fork().first();
    o1.subscribe(|v| value.set(*v));
    assert_eq!(value.get(), 1);

    value.set(0);
    o2.subscribe(|v| value.set(*v));
    assert_eq!(value.get(), 1);
  }
  #[test]
  fn first_or_support_fork() {
    use crate::ops::Fork;
    let default = Cell::new(0);
    let o = Observable::new(|subscriber| {
      subscriber.complete();
      subscriber.error(&"");
    })
    .first_or(100);
    let o1 = o.fork().first_or(0);
    let o2 = o.fork().first_or(0);
    o1.subscribe(|v| default.set(*v));
    assert_eq!(default.get(), 100);

    default.set(0);
    o2.subscribe(|v| default.set(*v));
    assert_eq!(default.get(), 100);
  }
}
