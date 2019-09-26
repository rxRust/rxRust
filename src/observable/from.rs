use crate::prelude::*;

pub macro from_iter($iter:expr) {
  Observable::new(move |mut subscriber| {
    // only for infer type
    let _: &Observer<_, ()> = &subscriber;
    for v in $iter.into_iter() {
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

pub macro of($v: expr) {
  Observable::new(move |mut subscriber| {
    // only for infer type
    let _: &Observer<_, ()> = &subscriber;

    subscriber.next(&$v);
    subscriber.complete();
  })
}

pub macro empty() {
  Observable::new(move |mut subscriber: Subscriber<_, _>| {
    // only for infer type
    let _: &Observer<_, ()> = &subscriber;
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
    observable::from_iter!(0..100)
      .subscribe_complete(|_| hit_count += 1, || completed = true);

    assert_eq!(hit_count, 100);
    assert_eq!(completed, true);
  }

  #[test]
  fn from_vec() {
    let mut hit_count = 0;
    let mut completed = false;
    observable::from_iter!(vec![0; 100])
      .subscribe_complete(|_| hit_count += 1, || completed = true);

    assert_eq!(hit_count, 100);
    assert_eq!(completed, true);
  }

  #[test]
  fn of() {
    let mut value = 0;
    let mut completed = false;
    observable::of!(100)
      .subscribe_complete(|v| value = *v, || completed = true);

    assert_eq!(value, 100);
    assert_eq!(completed, true);
  }

  #[test]
  fn empty() {
    let mut hits = 0;
    let mut completed = false;
    observable::empty!()
      .subscribe_complete(|_: &()| hits += 1, || completed = true);

    assert_eq!(hits, 0);
    assert_eq!(completed, true);
  }

  #[test]
  fn fork() {
    use crate::ops::Fork;

    observable::from_iter!(vec![0; 100])
      .fork()
      .fork()
      .subscribe(|_| {});

    observable::of!(0).fork().fork().subscribe(|_| {});
  }
}
