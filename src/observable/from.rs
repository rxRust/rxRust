use crate::prelude::*;

pub macro from_iter($iter:expr) {
  Observable::new(move |subscriber: &mut dyn Observer<_, ()>| {
    $iter
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

pub macro of($v: expr) {
  Observable::new(move |subscriber: &mut dyn Observer<_, ()>| {
    subscriber.next(&$v);
    subscriber.complete();
  })
}

pub macro empty() {
  Observable::new(move |subscriber: &mut dyn Observer<(), ()>| {
    subscriber.complete()
  })
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
  };

  #[test]
  fn from_range() {
    let hit_count = Arc::new(Mutex::new(0));
    let completed = Arc::new(AtomicBool::new(false));
    let c_hit_count = hit_count.clone();
    let c_completed = completed.clone();
    observable::from_iter!(0..100).subscribe_complete(
      move |_| *hit_count.lock().unwrap() += 1,
      move || completed.store(true, Ordering::Relaxed),
    );

    assert_eq!(*c_hit_count.lock().unwrap(), 100);
    assert_eq!(c_completed.load(Ordering::Relaxed), true);
  }

  #[test]
  fn from_vec() {
    let hit_count = Arc::new(Mutex::new(0));
    let completed = Arc::new(AtomicBool::new(false));
    let c_hit_count = hit_count.clone();
    let c_completed = completed.clone();
    observable::from_iter!(vec![0; 100]).subscribe_complete(
      move |_| *hit_count.lock().unwrap() += 1,
      move || completed.store(true, Ordering::Relaxed),
    );

    assert_eq!(*c_hit_count.lock().unwrap(), 100);
    assert_eq!(c_completed.load(Ordering::Relaxed), true);
  }

  #[test]
  fn of() {
    let value = Arc::new(Mutex::new(0));
    let completed = Arc::new(AtomicBool::new(false));
    let c_value = value.clone();
    let c_completed = completed.clone();
    observable::of!(100).subscribe_complete(
      move |v| *value.lock().unwrap() = *v,
      move || completed.store(true, Ordering::Relaxed),
    );

    assert_eq!(*c_value.lock().unwrap(), 100);
    assert_eq!(c_completed.load(Ordering::Relaxed), true);
  }

  #[test]
  fn empty() {
    let hits = Arc::new(Mutex::new(0));
    let completed = Arc::new(AtomicBool::new(false));
    let c_hits = hits.clone();
    let c_completed = completed.clone();
    observable::empty!().subscribe_complete(
      move |_| *hits.lock().unwrap() += 1,
      move || completed.store(true, Ordering::Relaxed),
    );

    assert_eq!(*c_hits.lock().unwrap(), 0);
    assert_eq!(c_completed.load(Ordering::Relaxed), true);
  }

  #[test]
  fn fork() {
    use crate::ops::{Fork, Multicast};

    observable::from_iter!(vec![0; 100])
      .multicast()
      .fork()
      .multicast()
      .fork()
      .subscribe(|_| {});

    observable::of!(0)
      .multicast()
      .fork()
      .multicast()
      .fork()
      .subscribe(|_| {});
  }
}
