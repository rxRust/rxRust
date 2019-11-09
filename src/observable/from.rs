use crate::prelude::*;

pub fn from_iter<O, U, Iter>(
  iter: Iter,
) -> Observable<impl FnOnce(Subscriber<O, U>) + Clone>
where
  O: Observer<<Iter as IntoIterator>::Item, ()>,
  U: SubscriptionLike,
  Iter: IntoIterator + Clone,
{
  Observable::new(move |mut subscriber| {
    for v in iter.into_iter() {
      if !subscriber.is_closed() {
        subscriber.next(&v);
      } else {
        break;
      }
    }
    if !subscriber.is_closed() {
      subscriber.complete();
    }
  })
}

pub fn of<O, U, Item>(
  v: Item,
) -> Observable<impl FnOnce(Subscriber<O, U>) + Clone>
where
  O: Observer<Item, ()>,
  U: SubscriptionLike,
  Item: Clone,
{
  Observable::new(move |mut subscriber| {
    subscriber.next(&v);
    subscriber.complete();
  })
}

pub fn empty<O, U, Item>() -> Observable<impl FnOnce(Subscriber<O, U>) + Clone>
where
  O: Observer<Item, ()>,
  U: SubscriptionLike,
{
  Observable::new(move |mut subscriber: Subscriber<O, U>| {
    subscriber.complete();
  })
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn from_range() {
    let mut hit_count = 0;
    let mut completed = false;
    observable::from_iter(0..100)
      .subscribe_complete(|_| hit_count += 1, || completed = true);

    assert_eq!(hit_count, 100);
    assert_eq!(completed, true);
  }

  #[test]
  fn from_vec() {
    let mut hit_count = 0;
    let mut completed = false;
    observable::from_iter(vec![0; 100])
      .subscribe_complete(|_| hit_count += 1, || completed = true);

    assert_eq!(hit_count, 100);
    assert_eq!(completed, true);
  }

  #[test]
  fn of() {
    let mut value = 0;
    let mut completed = false;
    observable::of(100).subscribe_complete(|v| value = *v, || completed = true);

    assert_eq!(value, 100);
    assert_eq!(completed, true);
  }

  #[test]
  fn empty() {
    let mut hits = 0;
    let mut completed = false;
    observable::empty()
      .subscribe_complete(|_: &()| hits += 1, || completed = true);

    assert_eq!(hits, 0);
    assert_eq!(completed, true);
  }

  #[test]
  fn fork() {
    use crate::ops::Fork;

    observable::from_iter(vec![0; 100])
      .fork()
      .fork()
      .subscribe(|_| {});

    observable::of(0).fork().fork().subscribe(|_| {});
  }
}
