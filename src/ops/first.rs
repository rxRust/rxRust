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

impl<'a, O> First for O where O: Observable<'a> {}

/// emit only the first item (or a default item) emitted by an Observable
pub trait FirstOr<'a> {
  fn first_or(self, default: Self::Item) -> FirstOrOp<TakeOp<Self>, Self::Item>
  where
    Self: Observable<'a>,
  {
    FirstOrOp {
      source: self.first(),
      default: Some(default),
    }
  }
}

impl<'a, O> FirstOr<'a> for O where O: Observable<'a> {}

pub struct FirstOrOp<S, V> {
  source: S,
  default: Option<V>,
}

impl<'a, S, T> Observable<'a> for FirstOrOp<S, T>
where
  T: 'a,
  S: Observable<'a, Item = T>,
{
  type Item = S::Item;
  type Err = S::Err;
  type Unsubscribe = S::Unsubscribe;

  fn subscribe_return_state<N>(self, next: N) -> Self::Unsubscribe
  where
    N: 'a + FnMut(&Self::Item) -> OState<Self::Err>,
  {
    let next = Rc::new(RefCell::new(next));
    let c_next = next.clone();
    let Self { source, default } = self;
    let default = Rc::new(RefCell::new(default));
    let c_default = default.clone();
    let mut subscription = source.subscribe_return_state(move |v| {
      c_default.borrow_mut().take();
      (&mut *(c_next.borrow_mut()))(v)
    });

    subscription.on_complete(move || {
      let default = default.borrow_mut().take();
      if let Some(d) = default {
        (&mut *(next.borrow_mut()))(&d);
      }
    });
    subscription
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
    numbers
      .clone()
      .first()
      .subscribe(|_| next_count.set(next_count.get() + 1))
      .on_complete(|| completed.set(true));

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

    let numbers = Subject::<'_, i32, ()>::new();
    numbers
      .clone()
      .first_or(100)
      .subscribe(|_| {
        next_count.set(next_count.get() + 1);
      })
      .on_complete(|| {
        completed.set(true);
      });

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
      .subscribe(|value| v.set(*value))
      .on_complete(|| completed.set(true));

    numbers.complete();
    assert_eq!(completed.get(), true);
    assert_eq!(v.get(), 100);
  }
}
