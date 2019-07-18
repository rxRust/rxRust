use crate::prelude::*;

pub fn from_iter<Item>(
  iter: impl IntoIterator<Item = Item> + Clone,
) -> Observable<impl Fn(&mut dyn Observer<Item = Item, Err = ()>), Item, ()> {
  Observable::new(move |subscriber| {
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
}

pub fn of<Item>(
  v: Item,
) -> Observable<impl Fn(&mut dyn Observer<Item = Item, Err = ()>), Item, ()> {
  Observable::new(move |subscriber| {
    subscriber.next(&v);
    subscriber.complete();
  })
}

pub fn empty<Item>()
-> Observable<impl Fn(&mut dyn Observer<Item = Item, Err = ()>), Item, ()> {
  Observable::new(move |subscriber| subscriber.complete())
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
  fn of() {
    let value = Cell::new(0);
    let completed = Cell::new(false);
    observable::of(100)
      .subscribe_complete(|v| value.set(*v), || completed.set(true));

    assert_eq!(value.get(), 100);
    assert_eq!(completed.get(), true);
  }

  #[test]
  fn empty() {
    let hits = Cell::new(0);
    let completed = Cell::new(false);
    observable::empty().subscribe_complete(
      |_: &i32| hits.set(hits.get() + 1),
      || completed.set(true),
    );

    assert_eq!(hits.get(), 0);
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

    observable::of(0)
      .fork()
      .filter(|_v| true)
      .fork()
      .subscribe(|_| {});
  }
}
