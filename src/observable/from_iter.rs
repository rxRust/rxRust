use crate::prelude::*;

pub fn from_iter<'a, T: 'a, Item: 'a, Err: 'a>(
  iter: T,
) -> impl Subscribable<'a, Item = Item, Err = Err>
where
  T: IntoIterator<Item = Item>,
{
  Observable::new(move |subcriber| {
    iter
      .into_iter()
      .take_while(|_| !subcriber.is_stopped())
      .for_each(|v| {
        subcriber.next(&v);
      });
    if !subcriber.is_stopped() {
      subcriber.complete();
    }
  })
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::cell::Cell;

  #[test]
  fn iter_to_observable() {
    let hit_count = Cell::new(0);
    let completed = Cell::new(false);
    observable::from_iter::<'_, _, _, ()>(0..100).subscribe_complete(
      |_| hit_count.set(hit_count.get() + 1),
      || completed.set(true),
    );

    assert_eq!(hit_count.get(), 100);
    assert_eq!(completed.get(), true);
  }
}
